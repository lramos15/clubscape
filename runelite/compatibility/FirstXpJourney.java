import clubscape.game.v1.Game;
import java.util.Comparator;
import java.util.function.Predicate;
import java.util.concurrent.Callable;

/** A bounded real input driver, stopping at first fishing XP; not a tutorial acceptance suite. */
final class FirstXpJourney
{
    private final ClubScapeTransport transport;
    private final WorldProjection projection;
    private final Evidence evidence;
    private final Callable<Boolean> baselineReady;

    FirstXpJourney(ClubScapeTransport transport, WorldProjection projection, Evidence evidence,
                   Callable<Boolean> baselineReady)
    {
        this.transport = transport;
        this.projection = projection;
        this.evidence = evidence;
        this.baselineReady = baselineReady;
    }

    private Game.WorldSnapshot poll() throws Exception
    {
        Thread.sleep(600);
        return transport.poll();
    }

    private void walk(int x, int y) throws Exception
    {
        transport.input(input -> input.setWalk(Game.Walk.newBuilder()
            .setDestination(Game.Tile.newBuilder().setX(x).setY(y).setPlane(0))));
        for (int ticks = 0; ticks < 80; ticks++)
        {
            Game.Player player = projection.snapshot().getPlayer();
            if (player.getTile().getX() == x && player.getTile().getY() == y) return;
            poll();
        }
        throw new IllegalStateException("Authoritative walking did not reach " + x + "," + y);
    }

    private void interact(String target, String action) throws Exception
    {
        Game.Entity entity = projection.entities().stream().filter(e -> e.getId().equals(target))
            .findFirst().orElseThrow(() -> new IllegalStateException("Target absent from real interest view: " + target));
        if (!entity.getActionsList().contains(action))
            throw new IllegalStateException("Action absent from typed source target view: " + action);
        transport.input(input -> input.setInteract(Game.Interact.newBuilder().setTarget(target).setAction(action)));
    }

    private void choose(String choice) throws Exception
    {
        Game.Dialogue dialogue = projection.snapshot().getDialogue();
        if (dialogue.getChoicesList().stream().noneMatch(c -> c.getId().equals(choice)))
            throw new IllegalStateException("Required choice not offered by the authoritative dialogue: " + choice);
        transport.input(input -> input.setDialogueChoice(Game.DialogueChoice.newBuilder()
            .setSpeaker(dialogue.getSpeaker()).setChoice(choice)));
    }

    private void open(String name) throws Exception
    {
        if (!projection.snapshot().getPlayer().getUnlockedInterfacesList().contains(name))
            throw new IllegalStateException("Interface not legitimately unlocked: " + name);
        transport.input(input -> input.setOpenInterface(Game.OpenInterface.newBuilder().setInterface(name)));
    }

    void run() throws Exception
    {
        Game.Player fresh = projection.snapshot().getPlayer();
        if (!fresh.getTutorialStage().equals("stage.tutorial.appearance")
            || fresh.getAppearanceConfirmed() || fresh.hasExperience() || fresh.getInventoryCount() != 0)
            throw new IllegalStateException("Not the real unseeded normal-account initial state");
        poll();
        poll();
        boolean initialized = false;
        for (int ticks = 0; ticks < 8; ticks++)
        {
            if (baselineReady.call()) { initialized = true; break; }
            poll();
        }
        if (!initialized) throw new IllegalStateException("Genuine XP Tracker did not initialize from actual server baseline");
        transport.input(input -> input.setConfirmAppearance(Game.ConfirmAppearance.newBuilder().putAppearance("body_type", 0)));
        transport.input(input -> input.setSelectExperience(Game.SelectExperience.newBuilder().setExperience("experience.brand_new")));
        interact("spawn.gielinor_guide", "Talk-to");
        choose("greeting_and_settings");
        open("interface.settings");
        transport.input(input -> input.setCloseInterface(Game.Empty.getDefaultInstance()));
        interact("spawn.gielinor_guide", "Talk-to");
        choose("settings_and_next_instructor");
        walk(3097, 3107);
        interact("spawn.tutorial.start_door.3098.3107.p0.t0.r0", "Open");
        walk(3098, 3107);
        if (!projection.snapshot().getPlayer().getTutorialStage().equals("stage.tutorial.survival_greeting"))
            throw new IllegalStateException("Real starting-door progression did not occur");
        walk(3102, 3095);
        interact("spawn.survival_expert", "Talk-to");
        choose("fishing_intro");
        open("interface.inventory");
        transport.input(input -> input.setCloseInterface(Game.Empty.getDefaultInstance()));
        walk(3103, 3093);
        Game.Entity fish = projection.entities().stream()
            .filter(e -> e.getDefinitionId().equals("npc.tutorial.fishing_spot"))
            .min(Comparator.comparingInt(e -> Math.abs(e.getTile().getX() - 3103) + Math.abs(e.getTile().getY() - 3093)))
            .orElseThrow(() -> new IllegalStateException("No live source fishing spot"));
        String action = fish.getActionsList().stream().filter(a -> a.toLowerCase().contains("net"))
            .findFirst().orElseThrow(() -> new IllegalStateException("Source fishing net interaction absent"));
        interact(fish.getId(), action);
        for (int ticks = 0; ticks < 180; ticks++)
        {
            if (projection.gainedXp().getOrDefault("skill.fishing", 0L) > 0)
            {
                transport.input(input -> input.setCancelActivity(Game.Empty.getDefaultInstance()));
                evidence.record("legitimate_first_xp", "skill", "skill.fishing",
                    "xp_tenths", projection.gainedXp().get("skill.fishing"),
                    "tutorial_stage", projection.snapshot().getPlayer().getTutorialStage(),
                    "full_tutorial_acceptance", false);
                return;
            }
            poll();
        }
        throw new IllegalStateException("No authoritative first fishing XP within the diagnostic bound");
    }
}
