import clubscape.account.v1.AccountOuterClass;
import clubscape.account.v1.AccountOuterClass.ClientMessage;
import clubscape.account.v1.AccountOuterClass.ServerMessage;
import clubscape.game.v1.Game;
import java.io.IOException;
import java.io.InputStream;
import java.net.InetSocketAddress;
import java.net.Proxy;
import java.net.ProxySelector;
import java.net.SocketAddress;
import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.security.MessageDigest;
import java.time.Duration;
import java.util.HexFormat;
import java.util.List;
import java.util.UUID;
import java.util.function.Consumer;

final class ClubScapeTransport implements AutoCloseable
{
    private final URI rpc;
    private final HttpClient http;
    private final Evidence evidence;
    private String token;
    private String session;
    private long sequence;
    private long characterRevision;
    private long revision;
    private String actor;
    private Consumer<Game.WorldSnapshot> consumer;

    ClubScapeTransport(URI origin, Evidence evidence)
    {
        if (!origin.getScheme().equals("http") || !origin.getHost().equals("127.0.0.1")
            || origin.getPort() <= 0 || origin.getRawUserInfo() != null
            || origin.getRawQuery() != null || origin.getRawFragment() != null
            || !(origin.getPath().isEmpty() || origin.getPath().equals("/")))
            throw new IllegalArgumentException("Only an explicit owned loopback origin is allowed");
        rpc = origin.resolve("/v1/rpc");
        this.evidence = evidence;
        http = HttpClient.newBuilder().connectTimeout(Duration.ofSeconds(5))
            .followRedirects(HttpClient.Redirect.NEVER)
            .proxy(new ProxySelector()
            {
                public List<Proxy> select(URI uri) { return List.of(Proxy.NO_PROXY); }
                public void connectFailed(URI uri, SocketAddress address, IOException error) {}
            }).build();
    }

    private ServerMessage request(ClientMessage.Builder builder) throws Exception
    {
        String requestId = UUID.randomUUID().toString();
        ClientMessage command = builder.setProtocolVersion(1).setRequestId(requestId).build();
        byte[] bytes = command.toByteArray();
        if (bytes.length > 16 * 1024) throw new IllegalArgumentException("Request exceeds protocol bound");
        HttpRequest.Builder request = HttpRequest.newBuilder(rpc).timeout(Duration.ofSeconds(12))
            .header("Content-Type", "application/x-protobuf")
            .header("Accept", "application/x-protobuf")
            .POST(HttpRequest.BodyPublishers.ofByteArray(bytes));
        if (token != null) request.header("Authorization", "Bearer " + token);
        long started = System.nanoTime();
        HttpResponse<InputStream> response = http.send(request.build(), HttpResponse.BodyHandlers.ofInputStream());
        byte[] result;
        try (InputStream body = response.body())
        {
            result = body.readNBytes(256 * 1024 + 1);
        }
        if (result.length > 256 * 1024) throw new IOException("Response exceeds protocol byte bound");
        if (!response.headers().firstValue("Content-Type").orElse("").startsWith("application/x-protobuf"))
            throw new IOException("Unexpected server response media type/status: " + response.statusCode());
        ServerMessage message = ServerMessage.parseFrom(result);
        if (message.getProtocolVersion() != 1 || !message.getRequestId().equals(requestId))
            throw new IOException("Server response identity/version mismatch");
        boolean secret = command.hasLogin() || command.hasRegister();
        evidence.record("rpc", "command", command.getCommandCase().name(), "request_id", requestId,
            "status", response.statusCode(), "result", message.getResultCase().name(),
            "elapsed_ms", (System.nanoTime() - started) / 1_000_000,
            "response_bytes", result.length,
            "response_sha256", secret ? "omitted: authentication material" :
                HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(result)));
        if (message.hasError())
        {
            evidence.record("server_denial", "code", message.getError().getCode().name(),
                "message", message.getError().getMessage());
            throw new IOException("Real server rejected " + command.getCommandCase() + ": "
                + message.getError().getCode() + ": " + message.getError().getMessage());
        }
        if (response.statusCode() != 200) throw new IOException("Non-success HTTP status");
        return message;
    }

    void signupAndJoin(Consumer<Game.WorldSnapshot> consumer, String expectedContentRevision) throws Exception
    {
        this.consumer = consumer;
        ServerMessage hello = request(ClientMessage.newBuilder().setHello(AccountOuterClass.Hello.getDefaultInstance()));
        if (hello.hasHello())
            evidence.record("server_hello", "gameplay_available", hello.getHello().getGameplayAvailable(),
                "capabilities", hello.getHello().getCapabilitiesList(),
                "gameplay_unavailable_reason", hello.getHello().getGameplayUnavailableReason(),
                "build", hello.getHello().getBuildRevision());
        if (!hello.hasHello() || !hello.getHello().getGameplayAvailable()
            || !hello.getHello().getCapabilitiesList().contains("game.v1"))
            throw new IllegalStateException("Actual server is not gameplay-ready");
        String name = "rl_" + UUID.randomUUID().toString().replace("-", "").substring(0, 14);
        String password = UUID.randomUUID().toString() + UUID.randomUUID();
        ServerMessage registered = request(ClientMessage.newBuilder().setRegister(
            AccountOuterClass.Register.newBuilder().setLoginName(name).setPassword(password)));
        if (!registered.hasRegistered()) throw new IOException("Expected registration");
        ServerMessage login = request(ClientMessage.newBuilder().setLogin(
            AccountOuterClass.Login.newBuilder().setLoginName(name).setPassword(password)));
        password = null;
        if (!login.hasLoggedIn()) throw new IOException("Expected login");
        token = login.getLoggedIn().getSessionToken();
        ServerMessage created = request(ClientMessage.newBuilder()
            .setCreateCharacter(Game.CreateCharacter.getDefaultInstance()));
        if (!created.hasCharacterCreated()) throw new IOException("Expected empty-options source character creation");
        if (!created.getCharacterCreated().getContentRevision().equals(expectedContentRevision))
            throw new IOException("Authoritative character source revision differs from the selected original catalog");
        actor = created.getCharacterCreated().getActorId();
        ServerMessage joined = request(ClientMessage.newBuilder().setJoinWorld(Game.JoinWorld.getDefaultInstance()));
        if (!joined.hasWorldJoined()) throw new IOException("Expected actual world join");
        if (!joined.getWorldJoined().getContentRevision().equals(expectedContentRevision))
            throw new IOException("Authoritative world source revision differs from the selected original catalog");
        session = joined.getWorldJoined().getWorldSessionId();
        sequence = joined.getWorldJoined().getNextSequence();
        evidence.record("joined", "origin", rpc.getScheme() + "://" + rpc.getAuthority(),
            "actor_id", actor, "content_revision", joined.getWorldJoined().getContentRevision(),
            "next_sequence", sequence, "server_build", hello.getHello().getBuildRevision(),
            "account_kind", "fresh synthetic ClubScape account; no external account");
        accept(joined.getWorldJoined().getSnapshot());
    }

    private void accept(Game.WorldSnapshot snapshot)
    {
        if (!snapshot.getPlayer().getActorId().equals(actor))
            throw new IllegalStateException("Server changed the local actor");
        revision = snapshot.getRevision();
        characterRevision = snapshot.getCharacterRevision();
        sequence = snapshot.getNextSequence();
        consumer.accept(snapshot);
    }

    Game.WorldSnapshot poll() throws Exception
    {
        ServerMessage result = request(ClientMessage.newBuilder().setPollWorld(Game.PollWorld.newBuilder()
            .setWorldSessionId(session).setAfterRevision(revision)));
        if (!result.hasWorldSnapshot()) throw new IOException("Expected world snapshot");
        accept(result.getWorldSnapshot());
        return result.getWorldSnapshot();
    }

    Game.WorldSnapshot input(Consumer<Game.WorldInput.Builder> action) throws Exception
    {
        Game.WorldInput.Builder input = Game.WorldInput.newBuilder().setWorldSessionId(session)
            .setSequence(sequence).setExpectedCharacterRevision(characterRevision);
        action.accept(input);
        evidence.record("input", "sequence", sequence, "action", input.getActionCase().name(),
            "target", input.hasInteract() ? input.getInteract().getTarget() :
                input.hasDialogueChoice() ? input.getDialogueChoice().getChoice() :
                input.hasOpenInterface() ? input.getOpenInterface().getInterface() : "",
            "walk", input.hasWalk() ? Evidence.fields("x", input.getWalk().getDestination().getX(),
                "y", input.getWalk().getDestination().getY()) : null);
        ServerMessage result = request(ClientMessage.newBuilder().setWorldInput(input));
        if (!result.hasActionResult()) throw new IOException("Expected committed action result");
        if (result.getActionResult().getSequence() != sequence)
            throw new IOException("Action receipt sequence mismatch");
        accept(result.getActionResult().getSnapshot());
        return result.getActionResult().getSnapshot();
    }

    public void close() throws Exception
    {
        if (session != null)
        {
            request(ClientMessage.newBuilder().setLeaveWorld(Game.LeaveWorld.newBuilder().setWorldSessionId(session)));
            session = null;
        }
        if (token != null)
        {
            request(ClientMessage.newBuilder().setLogout(AccountOuterClass.Logout.getDefaultInstance()));
            token = null;
        }
    }
}
