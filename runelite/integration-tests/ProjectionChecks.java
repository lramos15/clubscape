import clubscape.account.v1.AccountOuterClass;
import clubscape.game.v1.Game;
import java.util.Arrays;
import java.util.List;
import java.util.UUID;

/** Synthetic codec/projection checks only; these never establish live compatibility. */
public final class ProjectionChecks
{
    private static final String ACTOR = "00000000-0000-4000-8000-000000000012";
    private static int checks;

    private static Game.WorldSnapshot state(long revision, long xp, boolean full, Game.Event... events)
    {
        return Game.WorldSnapshot.newBuilder().setRevision(revision).setTick(revision)
            .setNextSequence(1).setFullSnapshot(full)
            .setPlayer(Game.Player.newBuilder().setActorId(ACTOR)
                .setTile(Game.Tile.newBuilder().setX(3094).setY(3106))
                .addSkills(Game.Skill.newBuilder().setId("skill.fishing")
                    .setXpTenths(xp).setBaseLevel(1).setCurrentLevel(1)))
            .addAllEvents(Arrays.asList(events)).build();
    }

    private static void reject(Runnable operation)
    {
        try { operation.run(); }
        catch (IllegalArgumentException | IllegalStateException expected) { checks++; return; }
        throw new AssertionError("Invalid projection accepted");
    }

    public static void main(String[] args) throws Exception
    {
        String id = "00000000-0000-4000-8000-000000000001";
        byte[] hello = AccountOuterClass.ClientMessage.newBuilder().setProtocolVersion(1).setRequestId(id)
            .setHello(AccountOuterClass.Hello.getDefaultInstance()).build().toByteArray();
        byte[] expected = new byte[42];
        expected[0] = 8; expected[1] = 1; expected[2] = 18; expected[3] = 36;
        System.arraycopy(id.getBytes(java.nio.charset.StandardCharsets.US_ASCII), 0, expected, 4, 36);
        expected[40] = 82;
        if (!Arrays.equals(hello, expected)) throw new AssertionError("Existing account v1 tags changed");
        checks++;
        var game = AccountOuterClass.ClientMessage.newBuilder().setProtocolVersion(1).setRequestId(id)
            .setPollWorld(Game.PollWorld.newBuilder().setWorldSessionId(id).setAfterRevision(12)).build();
        if (!AccountOuterClass.ClientMessage.parseFrom(game.toByteArray()).equals(game))
            throw new AssertionError("Generated game schema roundtrip");
        checks++;
        WorldProjection projection = new WorldProjection();
        reject(() -> projection.accept(state(1, 0, false)));
        projection.accept(state(1, 0, true));
        Game.Event event = Game.Event.newBuilder().setEventId("committed-event-1").setActorId(ACTOR)
            .setKind("xp_gained").setSkill("skill.fishing").setXpTenths(100).build();
        if (projection.accept(state(2, 100, false, event)).size() != 1)
            throw new AssertionError("New committed event lost");
        if (!projection.accept(state(2, 100, false, event)).isEmpty()
            || projection.gainedXp().get("skill.fishing") != 100)
            throw new AssertionError("Duplicate event counted twice");
        checks++;
        reject(() -> projection.accept(state(1, 100, false)));
        WorldProjection missing = new WorldProjection();
        missing.accept(state(1, 0, true));
        reject(() -> missing.accept(state(2, 100, false)));
        WorldProjection gap = new WorldProjection();
        gap.accept(state(1, 0, true));
        reject(() -> gap.accept(state(2, 0, false).toBuilder().setEventHistoryGap(true).build()));
        WorldProjection privacy = new WorldProjection();
        privacy.accept(state(1, 0, true));
        reject(() -> privacy.accept(state(2, 100, false,
            event.toBuilder().setActorId(UUID.randomUUID().toString()).build())));
        Game.Entity npc = Game.Entity.newBuilder().setId("npc").setKind(Game.EntityKind.NPC).build();
        WorldProjection entities = new WorldProjection();
        entities.accept(state(1, 0, true).toBuilder().addEntities(npc).build());
        entities.accept(state(2, 0, false).toBuilder().addRemovedEntities("npc").build());
        if (!entities.entities().isEmpty()) throw new AssertionError("Removal did not clear entity baseline");
        checks++;
        System.out.println("{\"synthetic_projection_checks\":" + checks + ",\"live_compatibility_proof\":false}");
    }
}
