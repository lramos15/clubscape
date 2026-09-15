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
    private long tick;
    private long lastInputTick = -1;
    private String actor;
    private Consumer<Game.WorldSnapshot> consumer;
    private final PendingWorldInput pendingInput = new PendingWorldInput();

    static final class ServerRejection extends IOException
    {
        final AccountOuterClass.ErrorCode code;
        ServerRejection(AccountOuterClass.Error error)
        {
            super("Real server rejected the request: " + error.getCode() + ": " + error.getMessage());
            code = error.getCode();
        }
    }

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
        ClientMessage command = identify(builder);
        validateNewShopRequest(command);
        return send(command);
    }

    private static ClientMessage identify(ClientMessage.Builder builder)
    {
        return builder.setProtocolVersion(1).setRequestId(UUID.randomUUID().toString()).build();
    }

    static void validateNewShopRequest(ClientMessage command)
    {
        if (command.hasWorldInput() && command.getWorldInput().hasShopBuy())
            ShopSelection.requireIdentity(command.getWorldInput().getShopBuy());
        if (command.hasPollWorld() && command.getPollWorld().hasQuote()
            && command.getPollWorld().getQuote().hasShopBuy())
            ShopSelection.requireIdentity(command.getPollWorld().getQuote().getShopBuy());
    }

    private ServerMessage send(ClientMessage command) throws Exception
    {
        String requestId = command.getRequestId();
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
            throw new ServerRejection(message.getError());
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
        String name = "rl_" + UUID.randomUUID().toString().replace("-", "").substring(0, 8);
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
        tick = snapshot.getTick();
        characterRevision = snapshot.getCharacterRevision();
        sequence = snapshot.getNextSequence();
        consumer.accept(snapshot);
    }

    Game.WorldSnapshot poll() throws Exception
    {
        return poll(null);
    }

    private Game.WorldSnapshot poll(Game.QuoteRequest quote) throws Exception
    {
        Game.PollWorld.Builder poll = Game.PollWorld.newBuilder()
            .setWorldSessionId(session).setAfterRevision(revision);
        if (quote != null) poll.setQuote(quote);
        ServerMessage result = request(ClientMessage.newBuilder().setPollWorld(poll));
        if (!result.hasWorldSnapshot()) throw new IOException("Expected world snapshot");
        accept(result.getWorldSnapshot());
        return result.getWorldSnapshot();
    }

    Game.WorldSnapshot buyDisplayedRow(ShopSelection selection) throws Exception
    {
        return input(input -> input.setShopBuy(selection.buy()));
    }

    Game.WorldSnapshot quoteDisplayedRow(ShopSelection selection) throws Exception
    {
        try { return poll(selection.quote()); }
        catch (ServerRejection rejected)
        {
            if (rejected.code == AccountOuterClass.ErrorCode.CONFLICT)
            {
                boolean refreshed = refreshRejectedShop(rejected);
                evidence.record("shop_selection_rejected", "read_only_quote", true,
                    "expected_item", selection.buy().getExpectedItem(),
                    "view_refreshed", refreshed,
                    "action_required", refreshed ? "Choose an item from the refreshed displayed view; no automatic identity substitution"
                        : "Refresh the view before a new choice; retain the original uncertain selection");
            }
            throw rejected;
        }
    }

    Game.WorldSnapshot input(Consumer<Game.WorldInput.Builder> action) throws Exception
    {
        if (pendingInput.active())
            throw new IllegalStateException("Retry or reconcile the retained operation; do not build a new intent");
        for (int waits = 0; tick <= lastInputTick; waits++)
        {
            if (waits >= 10) throw new IllegalStateException("Authoritative tick did not advance for the next input");
            Thread.sleep(600);
            poll();
        }
        Game.WorldInput.Builder input = Game.WorldInput.newBuilder().setWorldSessionId(session)
            .setSequence(sequence).setExpectedCharacterRevision(characterRevision);
        action.accept(input);
        evidence.record("input", "sequence", sequence, "action", input.getActionCase().name(),
            "target", input.hasInteract() ? input.getInteract().getTarget() :
                input.hasDialogueChoice() ? input.getDialogueChoice().getChoice() :
                input.hasOpenInterface() ? input.getOpenInterface().getInterface() : "",
            "walk", input.hasWalk() ? Evidence.fields("x", input.getWalk().getDestination().getX(),
                "y", input.getWalk().getDestination().getY()) : null);
        ClientMessage command = identify(ClientMessage.newBuilder().setWorldInput(input));
        validateNewShopRequest(command);
        pendingInput.begin(command);
        evidence.record("pending_world_input", "request_id", command.getRequestId(),
            "sequence", command.getWorldInput().getSequence(), "action", input.getActionCase().name(),
            "expected_item", input.hasShopBuy() ? input.getShopBuy().getExpectedItem() : "",
            "retry_policy", "Retain the exact original operation; never rebind a refreshed shop row");
        return submitPendingInput();
    }

    Game.WorldSnapshot retryPendingInput() throws Exception
    {
        return submitPendingInput();
    }

    private boolean refreshRejectedShop(ServerRejection rejection)
    {
        try { poll(); return true; }
        catch (Exception refreshFailure) { rejection.addSuppressed(refreshFailure); return false; }
    }

    private Game.WorldSnapshot submitPendingInput() throws Exception
    {
        ClientMessage command = pendingInput.original();
        ServerMessage result;
        try { result = send(command); }
        catch (ServerRejection rejected)
        {
            if (command.getWorldInput().hasShopBuy()
                && rejected.code == AccountOuterClass.ErrorCode.CONFLICT)
            {
                long beforeRefresh = revision;
                try
                {
                    Game.WorldSnapshot refreshed = poll();
                    boolean notConsumed = pendingInput.rejectedWithoutConsumption(refreshed.getNextSequence());
                    evidence.record("shop_selection_rejected", "request_id", command.getRequestId(),
                        "expected_item", command.getWorldInput().getShopBuy().getExpectedItem(),
                        "refreshed_revision", refreshed.getRevision(), "previous_revision", beforeRefresh,
                        "sequence_not_consumed", notConsumed,
                        "action_required", notConsumed ? "Choose an item from the refreshed displayed view"
                            : "Outcome requires reconciliation; retain the original operation bytes");
                }
                catch (Exception refreshFailure)
                {
                    rejected.addSuppressed(refreshFailure);
                    evidence.record("shop_refresh_failed", "request_id", command.getRequestId(),
                        "selection_retained", true, "outcome_reconciled", false);
                }
            }
            throw rejected;
        }
        if (!result.hasActionResult()) throw new IOException("Expected committed action result");
        if (result.getActionResult().getSequence() != command.getWorldInput().getSequence()
            || !result.getActionResult().getOperationId().equals(command.getRequestId()))
            throw new IOException("Action receipt sequence mismatch");
        pendingInput.acknowledged(result.getActionResult().getSequence());
        lastInputTick = result.getActionResult().getSnapshot().getTick();
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
