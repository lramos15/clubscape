import clubscape.game.v1.Game;
import com.google.gson.JsonObject;
import com.google.inject.Guice;
import com.google.inject.Injector;
import java.awt.Dimension;
import java.awt.Rectangle;
import java.awt.Robot;
import java.awt.Window;
import java.lang.reflect.Field;
import java.net.URI;
import java.net.URL;
import java.nio.charset.Charset;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import javax.imageio.ImageIO;
import javax.swing.SwingUtilities;
import net.runelite.api.ClientConfiguration;
import net.runelite.api.GameState;
import net.runelite.api.Skill;
import net.runelite.api.events.GameStateChanged;
import net.runelite.api.events.GameTick;
import net.runelite.api.events.StatChanged;
import net.runelite.client.RuneLite;
import net.runelite.client.RuneLiteModule;
import net.runelite.client.RuneLiteProperties;
import net.runelite.client.RuntimeConfigLoader;
import net.runelite.client.config.ConfigManager;
import net.runelite.client.eventbus.EventBus;
import net.runelite.client.plugins.PluginManager;
import net.runelite.client.plugins.xptracker.XpTrackerPlugin;
import net.runelite.client.plugins.xptracker.XpTrackerReadback;
import net.runelite.client.ui.ClientToolbar;
import net.runelite.client.ui.ClientUI;
import net.runelite.client.ui.NavigationButton;
import net.runelite.client.ui.overlay.OverlayManager;
import net.runelite.http.api.RuneLiteAPI;
import okhttp3.OkHttpClient;

/** Official RuneLite composition root with ClubScape transport/state, not a replacement desktop UI. */
public final class RuneLiteComposition
{
    private final Evidence evidence;
    private final client game = new client();
    private final WorldProjection projection = new WorldProjection();
    private final ArrayBlockingQueue<Runnable> work = new ArrayBlockingQueue<>(64);
    private Injector injector;
    private NativeScene scene;
    private XpTrackerPlugin tracker;
    private JsonObject catalog;
    private boolean joined;
    private long lastTick = -1;
    private final Map<String, Long> initialXp = new LinkedHashMap<>();
    private Path liveOutput;
    private Game.Tile previousTile;
    private int authoritativeMoves;
    private boolean baselineVerified;

    private RuneLiteComposition(Evidence evidence) { this.evidence = evidence; }

    private void start(Path root, Path artifacts, VerifiedCache cache, Path catalogPath) throws Exception
    {
        catalog = Evidence.JSON.fromJson(Files.readString(catalogPath), JsonObject.class);
        OkHttpClient http = new OkHttpClient.Builder()
            .addInterceptor(chain ->
            {
                throw new java.io.IOException("External RuneLite HTTP is disabled in this isolated ClubScape experiment");
            }).build();
        RuneLiteAPI.CLIENT = http;
        System.setProperty("runelite.rtconf", root.resolve("tools/runelite-compatibility/runtime-config.json").toString());
        RuntimeConfigLoader runtimeConfig = new RuntimeConfigLoader(http);
        injector = Guice.createInjector(new RuneLiteModule(http, () -> game, runtimeConfig,
            false, true, true, Path.of(System.getProperty("user.home"), "no-external-session").toFile(),
            null, false, true));
        RuneLite.setInjector(injector);
        injector.injectMembers(game);
        Map<String, String> parameters = new LinkedHashMap<>();
        String codebase = null;
        for (String line : Files.readAllLines(root.resolve("research/current-source/runtime-config.ws"),
            Charset.forName("windows-1252")))
        {
            if (line.startsWith("param="))
            {
                String[] pair = line.substring(6).split("=", 2);
                parameters.put(pair[0], pair[1]);
            }
            else if (line.startsWith("codebase=")) codebase = line.substring(9);
        }
        URL base = new URL(codebase);
        game.setConfiguration(new ClientConfiguration()
        {
            public URL getCodeBase() { return base; }
            public String getParameter(String name) { return parameters.get(name); }
            public void onError(String code) { throw new IllegalStateException("Original initialization error: " + code); }
        });
        int before = game.getRevision();
        game.setSize(1280, 800);
        System.setProperty("runelite.delaystart", "true");
        game.init();
        if (before != 0 || game.getRevision() != 240)
            throw new IllegalStateException("Original init revision differs from the pinned runtime");
        Thread originalThread = null;
        long startupDeadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(20);
        while (System.nanoTime() < startupDeadline)
        {
            originalThread = game.getClientThread();
            if (originalThread != null && originalThread.getState() == Thread.State.WAITING
                && Arrays.stream(originalThread.getStackTrace()).anyMatch(
                    frame -> frame.getClassName().equals("java.util.concurrent.Semaphore")
                        && frame.getMethodName().equals("acquire")))
                break;
            Thread.sleep(20);
        }
        if (originalThread == null || originalThread.getState() != Thread.State.WAITING)
            throw new IllegalStateException("Original client did not stop at its supported delaystart latch");
        evidence.record("original_init", "before", before, "after", game.getRevision(),
            "build_id", game.getBuildID(), "runelite_version", RuneLiteProperties.getVersion(),
            "runtime_class", game.getClass().getName(),
            "runtime_jar", game.getClass().getProtectionDomain().getCodeSource().getLocation().toString(),
            "user_home", System.getProperty("user.home"),
            "callbacks", game.getCallbacks().getClass().getName(),
            "startup_latch", "unchanged tq.run / runelite.delaystart / Semaphore.acquire before OSRS tick/network pump",
            "original_thread_state", originalThread.getState().name(),
            "original_osrs_network_pump_started", false);
        scene = new NativeScene(game, cache, catalog, evidence);
        ConfigManager config = injector.getInstance(ConfigManager.class);
        config.load();
        config.setConfiguration("runelite", "gameSize", new Dimension(1280, 800));
        config.setConfiguration("xptracker", "saveState", false);
        PluginManager plugins = injector.getInstance(PluginManager.class);
        List<net.runelite.client.plugins.Plugin> loaded = plugins.loadPlugins(List.of(XpTrackerPlugin.class), null);
        if (loaded.size() != 1) throw new IllegalStateException("Expected the one named genuine plugin");
        tracker = (XpTrackerPlugin) loaded.get(0);
        plugins.loadDefaultPluginConfiguration(null);
        ClientUI ui = injector.getInstance(ClientUI.class);
        ui.init();
        EventBus bus = injector.getInstance(EventBus.class);
        bus.register(ui);
        bus.register(plugins);
        bus.register(injector.getInstance(OverlayManager.class));
        bus.register(config);
        SwingUtilities.invokeAndWait(() ->
        {
            try
            {
                plugins.setPluginEnabled(tracker, true);
                if (!plugins.startPlugin(tracker)) throw new IllegalStateException("XP Tracker did not start");
            }
            catch (Exception error) { throw new IllegalStateException(error); }
        });
        ui.show();
        evidence.record("official_runtime_ui", "ui_class", ui.getClass().getName(),
            "plugin_class", tracker.getClass().getName(),
            "plugin_jar", tracker.getClass().getProtectionDomain().getCodeSource().getLocation().toString(),
            "plugin_active", plugins.isPluginActive(tracker), "upstream_modified", false,
            "original_entry_difference", "Official module/UI/plugin lifecycle composed directly; OSRS protocol pump intentionally not started");
    }

    private void gameState(GameState state) throws Exception
    {
        NativeFields.logicalInt(null, client.class, "ci", -44590225, state.getState());
        GameStateChanged event = new GameStateChanged();
        event.setGameState(state);
        game.getCallbacks().post(event);
    }

    private void apply(Game.WorldSnapshot snapshot)
    {
        CompletableFuture<Void> completed = new CompletableFuture<>();
        if (!work.offer(() ->
        {
            try
            {
                List<Game.Event> events = projection.accept(snapshot);
                Game.Player player = snapshot.getPlayer();
                if (!joined)
                {
                    gameState(GameState.LOGGING_IN);
                    player.getSkillsList().forEach(skill -> initialXp.put(skill.getId(), skill.getXpTenths()));
                    evidence.record("authoritative_initial_player", "actor_id", player.getActorId(),
                        "tick", snapshot.getTick(), "revision", snapshot.getRevision(),
                        "stage", player.getTutorialStage(), "x", player.getTile().getX(), "y", player.getTile().getY(),
                        "hitpoints", player.getHitpoints(), "prayer_points", player.getPrayerPoints(),
                        "run_energy", player.getRunEnergy(), "inventory_slots", player.getInventoryCount(),
                        "appearance_confirmed", player.getAppearanceConfirmed(), "experience_selected", player.hasExperience(),
                        "skills", player.getSkillsList().stream().map(skill -> Evidence.fields(
                            "id", skill.getId(), "xp_tenths", skill.getXpTenths(),
                            "base_level", skill.getBaseLevel(), "current_level", skill.getCurrentLevel())).toList());
                }
                for (Game.Skill skill : player.getSkillsList())
                {
                    Skill nativeSkill = Skill.valueOf(skill.getId().substring("skill.".length()).toUpperCase(Locale.ROOT));
                    int ordinal = nativeSkill.ordinal();
                    game.getSkillExperiences()[ordinal] = Math.toIntExact(skill.getXpTenths() / 10);
                    game.getRealSkillLevels()[ordinal] = skill.getBaseLevel();
                    game.getBoostedSkillLevels()[ordinal] = skill.getCurrentLevel();
                }
                JsonObject items = catalog.getAsJsonObject("items");
                for (int slot = 0; slot < 28; slot++) bh.aq(93, slot, -1, 0);
                for (Game.ItemSlot slot : player.getInventoryList())
                {
                    if (!items.has(slot.getStack().getItem()))
                        throw new IllegalStateException("Actual inventory item is missing its frozen source identity");
                    bh.aq(93, slot.getIndex(), items.get(slot.getStack().getItem()).getAsInt(),
                        slot.getStack().getQuantity());
                }
                scene.apply(player);
                if (previousTile != null && !previousTile.equals(player.getTile())) authoritativeMoves++;
                previousTile = player.getTile();
                if (!joined)
                {
                    gameState(GameState.LOGGED_IN);
                    joined = true;
                    if (liveOutput != null) screenshot(liveOutput, "live-joined");
                }
                for (Game.Event event : events)
                {
                    evidence.record("authoritative_event", "event_id", event.getEventId(), "event", event.getKind(),
                        "revision", snapshot.getRevision(), "tick", snapshot.getTick(), "skill", event.getSkill(),
                        "xp_tenths", event.getXpTenths(), "target", event.getTarget(), "text", event.getText());
                    if (event.getKind().equals("xp_gained"))
                    {
                        Skill skill = Skill.valueOf(event.getSkill().substring(6).toUpperCase(Locale.ROOT));
                        if (!baselineVerified || XpTrackerReadback.initializationTicksRemaining(tracker) != 0)
                            throw new IllegalStateException("Real XP arrived before the genuine tracker baseline was verified");
                        int xp = game.getSkillExperience(skill);
                        game.getCallbacks().post(new StatChanged(skill, xp,
                            game.getRealSkillLevel(skill), game.getBoostedSkillLevel(skill)));
                        evidence.record("native_stat_event", "server_event_id", event.getEventId(),
                            "skill", skill.name(), "native_absolute_xp", xp,
                            "server_gain_tenths", event.getXpTenths(), "integer_scale", 10,
                            "event_bus", "genuine net.runelite.client.callback.Hooks",
                            "tracker_gained", XpTrackerReadback.gained(tracker, skill));
                    }
                }
                if (snapshot.getTick() != lastTick)
                {
                    game.setTickCount(Math.toIntExact(snapshot.getTick()));
                    game.getCallbacks().post(new GameTick());
                    evidence.record("authoritative_game_tick", "tick", snapshot.getTick(), "previous_observed_tick", lastTick);
                    lastTick = snapshot.getTick();
                }
                evidence.record("projected_state", "tick", snapshot.getTick(), "revision", snapshot.getRevision(),
                    "character_revision", snapshot.getCharacterRevision(), "stage", player.getTutorialStage(),
                    "x", player.getTile().getX(), "y", player.getTile().getY(),
                    "inventory_slots", player.getInventoryCount(), "entities", projection.entities().size(),
                    "dialogue", snapshot.getDialogue().getId());
                completed.complete(null);
            }
            catch (Throwable error) { completed.completeExceptionally(error); }
        })) throw new IllegalStateException("Bounded native state queue is full");
        try { completed.get(45, TimeUnit.SECONDS); }
        catch (Exception error) { throw new IllegalStateException("Could not project actual server state", error); }
    }

    private boolean checkPluginBaseline() throws Exception
    {
        CompletableFuture<Boolean> completed = new CompletableFuture<>();
        if (!work.offer(() ->
        {
            try
            {
                long tenths = initialXp.get("skill.fishing");
                int integerXp = Math.toIntExact(tenths / 10);
                boolean ready = projection.gainedXp().isEmpty()
                    && game.getSkillExperience(Skill.FISHING) == integerXp
                    && XpTrackerReadback.baselineMatches(tracker, Skill.FISHING, integerXp);
                if (ready)
                {
                    baselineVerified = true;
                    evidence.record("plugin_baseline_verified", "server_tick", projection.snapshot().getTick(),
                        "server_revision", projection.snapshot().getRevision(), "source_xp_tenths", tenths,
                        "native_absolute_xp", integerXp, "tracker_gain", XpTrackerReadback.gained(tracker, Skill.FISHING),
                        "initialization_ticks_remaining", XpTrackerReadback.initializationTicksRemaining(tracker),
                        "fake_callbacks", 0);
                }
                completed.complete(ready);
            }
            catch (Throwable error) { completed.completeExceptionally(error); }
        })) throw new IllegalStateException("Bounded native baseline queue is full");
        return completed.get(15, TimeUnit.SECONDS);
    }

    private void showTracker(boolean overlay) throws Exception
    {
        if (overlay) XpTrackerReadback.showOverlay(tracker, Skill.FISHING);
        Field field = XpTrackerPlugin.class.getDeclaredField("navButton");
        field.setAccessible(true);
        NavigationButton navigation = (NavigationButton) field.get(tracker);
        SwingUtilities.invokeAndWait(() -> injector.getInstance(ClientToolbar.class).openPanel(navigation));
    }

    private void screenshot(Path output, String name) throws Exception
    {
        scene.render();
        ImageIO.write(scene.image(), "png", output.resolve(name + "-canvas.png").toFile());
        java.awt.Toolkit.getDefaultToolkit().sync();
        SwingUtilities.invokeAndWait(() ->
        {
            try
            {
                for (Window window : Window.getWindows())
                {
                    if (window.isShowing() && window.getWidth() > 500 && window.getHeight() > 300)
                    {
                        Rectangle rectangle = new Rectangle(window.getLocationOnScreen(), window.getSize());
                        ImageIO.write(new Robot().createScreenCapture(rectangle), "png",
                            output.resolve(name + "-official-window.png").toFile());
                    }
                }
                var rectangle = scene.screenRectangle();
                var screen = new Robot().createScreenCapture(new Rectangle(
                    rectangle.x(), rectangle.y(), rectangle.width(), rectangle.height()));
                var source = scene.image();
                long identical = 0, colored = 0;
                for (int y = 0; y < rectangle.height(); y++)
                    for (int x = 0; x < rectangle.width(); x++)
                    {
                        int displayed = screen.getRGB(x, y) & 0xffffff;
                        if (displayed == (source.getRGB(x, y) & 0xffffff)) identical++;
                        if (displayed != 0) colored++;
                    }
                ImageIO.write(screen, "png", output.resolve(name + "-screen-canvas.png").toFile());
                evidence.record("actual_window_pixel_readback", "equal_native_pixels", identical,
                    "total_pixels", (long) rectangle.width() * rectangle.height(), "nonblack_screen_pixels", colored,
                    "method", "AWT Robot readback from the real official RuneLite window; no PNG embedded in a viewport");
                if (colored < 10000 || identical < (long) rectangle.width() * rectangle.height() * 0.99)
                    throw new IllegalStateException("Actual official window does not show the native framebuffer");
            }
            catch (Exception error) { throw new IllegalStateException(error); }
        });
    }

    private void preflight(Path output) throws Exception
    {
        JsonObject tile = catalog.getAsJsonObject("developer_preflight_initial_tile");
        Game.Player developer = Game.Player.newBuilder()
            .setActorId("00000000-0000-4000-8000-000000000012")
            .setDisplayName("DeveloperOnly")
            .setTile(Game.Tile.newBuilder().setX(tile.get("x").getAsInt()).setY(tile.get("y").getAsInt()))
            .build();
        scene.apply(developer);
        NativeFields.logicalInt(null, client.class, "ci", -44590225, 30);
        showTracker(false);
        for (int frame = 0; frame < 30; frame++)
        {
            scene.render();
            Thread.sleep(40);
        }
        long pixels = scene.verifyVisibleActor("developer-only offline source fixture; no server connection");
        screenshot(output, "developer-only-preflight");
        evidence.record("developer_preflight_result", "native_actor_pixels", pixels,
            "official_ui_and_plugin_launched", true, "server_connected", false,
            "xp_events_posted", 0, "compatibility_verified", false);
        if (pixels <= 30) throw new IllegalStateException("Developer-only native penguin visibility preflight failed");
    }

    private void live(URI origin, Path output) throws Exception
    {
        liveOutput = output;
        CompletableFuture<Void> journey = new CompletableFuture<>();
        ExecutorService network = Executors.newSingleThreadExecutor(r -> new Thread(r, "ClubScape-HTTP"));
        network.submit(() ->
        {
            try (ClubScapeTransport transport = new ClubScapeTransport(origin, evidence))
            {
                transport.signupAndJoin(this::apply, catalog.get("content_revision").getAsString());
                new FirstXpJourney(transport, projection, evidence, this::checkPluginBaseline).run();
                journey.complete(null);
                // Leave/logout happen only after the main thread has preserved the complete live tuple.
                while (!network.isShutdown()) Thread.sleep(100);
            }
            catch (Throwable error) { journey.completeExceptionally(error); }
        });
        try
        {
            long deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(570);
            while (!journey.isDone())
            {
                Runnable pending;
                while ((pending = work.poll()) != null) pending.run();
                game.getCallbacks().tick();
                scene.render();
                if (System.nanoTime() > deadline)
                    throw new IllegalStateException("Bounded integration time exhausted; not proof of impossibility");
                Thread.sleep(40);
            }
            journey.get();
            long expectedTenths = projection.gainedXp().getOrDefault("skill.fishing", 0L);
            int expectedDisplayGain = Math.toIntExact((initialXp.get("skill.fishing") + expectedTenths) / 10
                - initialXp.get("skill.fishing") / 10);
            int observed = XpTrackerReadback.gained(tracker, Skill.FISHING);
            if (observed <= 0 || observed != expectedDisplayGain)
                throw new IllegalStateException("Genuine XP Tracker does not match authoritative committed experience");
            if (!baselineVerified || authoritativeMoves == 0)
                throw new IllegalStateException("Live baseline and actual authoritative movement were not demonstrated");
            showTracker(true);
            for (int frame = 0; frame < 12; frame++) { scene.render(); Thread.sleep(40); }
            long pixels = scene.verifyVisibleActor("real authoritative ClubScape snapshot");
            if (pixels <= 30) throw new IllegalStateException("Penguin is not visible in the actual live scene");
            screenshot(output, "live-first-xp");
            evidence.record("complete_tuple", "server_connected", true, "original_runtime", true,
                "native_scene", true, "penguin_visible_pixels", pixels, "tracker", tracker.getClass().getName(),
                "expected_plugin_xp", expectedDisplayGain, "observed_plugin_xp", observed,
                "authoritative_xp_tenths", expectedTenths,
                "native_absolute_xp", game.getSkillExperience(Skill.FISHING),
                "fractional_xp_remainder_tenths", (initialXp.get("skill.fishing") + expectedTenths) % 10,
                "display_conversion", "floor(current source xp_tenths / 10) - floor(initial source xp_tenths / 10); no source XP discarded",
                "baseline_verified_before_xp", baselineVerified, "authoritative_moves", authoritativeMoves,
                "final_server_tick", projection.snapshot().getTick(), "final_server_revision", projection.snapshot().getRevision(),
                "final_character_revision", projection.snapshot().getCharacterRevision(),
                "final_x", projection.snapshot().getPlayer().getTile().getX(),
                "final_y", projection.snapshot().getPlayer().getTile().getY(),
                "scope", "fresh account through first fishing XP only; not full M1 or broad plugin compatibility");
        }
        finally
        {
            network.shutdown();
            if (!network.awaitTermination(20, TimeUnit.SECONDS))
            {
                network.shutdownNow();
                network.awaitTermination(3, TimeUnit.SECONDS);
            }
        }
    }

    private void stop() throws Exception
    {
        if (injector == null) return;
        SwingUtilities.invokeAndWait(() ->
        {
            try
            {
                if (tracker != null) injector.getInstance(PluginManager.class).stopPlugin(tracker);
            }
            catch (Exception error) { throw new IllegalStateException(error); }
            finally { for (Window window : Window.getWindows()) window.dispose(); }
        });
        injector.getInstance(ScheduledExecutorService.class).shutdownNow();
    }

    public static void main(String[] args)
    {
        int exit = 1;
        Path root = Path.of(args[0]).toAbsolutePath();
        Path artifacts = root.resolve("runelite/compatibility/artifacts");
        Path output = Path.of(args[1]).toAbsolutePath();
        Path catalog = args.length > 3 ? Path.of(args[3]).toAbsolutePath() : artifacts.resolve("catalog.json");
        boolean preflight = args[2].equals("preflight");
        RuneLiteComposition runtime = null;
        try (Evidence evidence = new Evidence(output.resolve("events.jsonl"));
             VerifiedCache cache = new VerifiedCache(artifacts.resolve("cache-2695"), evidence))
        {
            Locale.setDefault(Locale.ENGLISH);
            ImageIO.setUseCache(false);
            Path home = Path.of(System.getProperty("user.home")).toAbsolutePath();
            if (!home.startsWith(artifacts) || !output.startsWith(artifacts) || !catalog.startsWith(artifacts))
                throw new IllegalArgumentException("Explicit Java user.home and output must be owned isolated paths");
            evidence.record("experiment_mode", "mode", preflight ? "developer-only offline preflight" : "live integration",
                "compatibility_verified", false, "upstream_runtime", "1.12.38", "architecture", "A");
            runtime = new RuneLiteComposition(evidence);
            runtime.start(root, artifacts, cache, catalog);
            if (preflight) runtime.preflight(output);
            else runtime.live(URI.create(args[2]), output);
            exit = 0;
        }
        catch (Throwable error) { error.printStackTrace(); }
        finally
        {
            try { if (runtime != null) runtime.stop(); }
            catch (Throwable error) { error.printStackTrace(); exit = 1; }
        }
        System.exit(exit);
    }
}
