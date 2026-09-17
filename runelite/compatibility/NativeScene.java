import clubscape.game.v1.Game;
import com.google.gson.JsonObject;
import java.awt.BorderLayout;
import java.awt.Canvas;
import java.awt.Dimension;
import java.awt.Graphics;
import java.awt.image.BufferedImage;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import net.runelite.api.Model;
import net.runelite.api.Scene;
import net.runelite.api.Tile;
import net.runelite.api.GameObject;
import net.runelite.api.Player;

final class NativeScene
{
    final client game;
    final VerifiedCache cache;
    final Evidence evidence;
    final Canvas canvas;
    private tg provider;
    private int width;
    private int height;
    private dz world;
    private ez scene;
    private ct player;
    private Game.Player lastPlayer;
    private int baseX;
    private int baseY;
    private int tileCount;
    private int objectCount;
    private final int penguinId;
    private final int idle;
    private final int walk;
    private final JsonObject catalog;
    private long frames;
    private long actorPixels;

    NativeScene(client game, VerifiedCache cache, JsonObject catalog, Evidence evidence) throws Exception
    {
        this.game = game;
        this.cache = cache;
        this.catalog = catalog;
        this.evidence = evidence;
        canvas = game.getCanvas();
        if (canvas == null)
            throw new IllegalStateException("Original delayed startup did not create its real canvas");
        JsonObject penguin = catalog.getAsJsonObject("penguin");
        penguinId = penguin.get("id").getAsInt();
        idle = penguin.get("standingAnimation").getAsInt();
        walk = penguin.get("walkingAnimation").getAsInt();
        if (penguinId != 2063 || penguin.getAsJsonArray("models").get(0).getAsInt() != 21547
            || penguin.get("widthScale").getAsInt() != 75 || penguin.get("heightScale").getAsInt() != 75)
            throw new IllegalArgumentException("The owner-approved original penguin tuple changed");
        oe.cz = game;
        game.km = Thread.currentThread();
        vp[] slots = (vp[]) NativeFields.field(client.class, "ov", vp[].class).get(null);
        for (int index : new int[]{0, 1, 2, 3, 5, 7, 8, 9, 10, 12, 13, 17, 22})
            if (index < slots.length) slots[index] = cache.archive(index);
        for (String[] binding : new String[][]{
            {"pl", "ae", "2"}, {"pl", "ab", "7"}, {"ot", "du", "2"}, {"gu", "dt", "7"},
            {"ou", "av", "2"}, {"kp", "at", "0"}, {"iy", "am", "1"}, {"gn", "an", "22"},
            {"kd", "cl", "7"}, {"ak", "cq", "2"}, {"pt", "ae", "2"},
            {"wn", "hs", "12"}, {"lh", "aa", "2"}, {"bf", "hu", "2"}
        })
        {
            var field = Class.forName(binding[0]).getDeclaredField(binding[1]);
            field.setAccessible(true);
            field.set(null, cache.archive(Integer.parseInt(binding[2])));
        }
        ec textures = new ec(cache.archive(9), cache.archive(8), 64, 0.8, 128);
        fh.af(textures);
        NativeFields.put(null, rs.class, "mr", ec.class, textures);
        fh.ae(0.8);
        int maximumVarp = 0;
        vp configurations = cache.archive(2);
        for (int id : configurations.getFileIds(14))
        {
            net.runelite.api.VarbitComposition varbit = new pt(new xy(configurations.loadData(14, id)));
            maximumVarp = Math.max(maximumVarp, varbit.getIndex());
        }
        NativeFields.put(null, lb.class, "af", int[].class, new int[maximumVarp + 1]);
        NativeFields.put(null, lb.class, "az", int[].class, new int[maximumVarp + 1]);
        for (var method : client.class.getDeclaredMethods())
        {
            if (method.getName().equals("al") && method.getReturnType() == void.class
                && method.getParameterCount() == 0 && Modifier.isStatic(method.getModifiers()))
            {
                method.setAccessible(true);
                method.invoke(null);
            }
        }
        vv widgets = new vv(cache.archive(3), cache.archive(7), cache.archive(8), cache.archive(13), null);
        NativeFields.put(null, wk.class, "cy", vv.class, widgets);
        ab.kn = new cy();
        NativeFields.put(null, at.class, "kt", aax.class, new MutedAnimationAudio());
        NativeFields.put(null, client.class, "ez", boolean.class, true);
        canvas.setSize(1280, 800);
        game.setLayout(new BorderLayout());
        game.add(canvas, BorderLayout.CENTER);
        game.setSize(1280, 800);
        game.setPreferredSize(new Dimension(1280, 800));
        evidence.record("native_canvas_binding", "class", canvas.getClass().getName(),
            "reused_from_original_startup", true, "client_children", game.getComponentCount(),
            "gpu", game.isGpu());
        fq.ab(32768);
        ez.cc(25);
        resize(1280, 800);
    }

    private void resize(int width, int height) throws Exception
    {
        this.width = width;
        this.height = height;
        provider = new tg(width, height, canvas, false);
        wo.qi = provider;
        NativeFields.logicalInt(null, sa.class, "qy", 773246731, width);
        NativeFields.logicalInt(null, eu.class, "qx", 8379747, height);
        bindPixels();
    }

    private void bindPixels()
    {
        fh.ap(provider.getPixels(), width, height, null);
        game.getRasterizer().resetRasterClipping();
        if (scene != null) rl.cu(0, 0, width, height, true, 317527437);
        else fh.ao.ok(512);
    }

    private void load(int focalX, int focalY) throws Exception
    {
        baseX = (focalX / 8 - 6) * 8;
        baseY = (focalY / 8 - 6) * 8;
        world = new dz(0, 104, 104, 25, ex.az);
        NativeFields.logicalInt(world, dz.class, "ac", -1444178379, baseX);
        NativeFields.logicalInt(world, dz.class, "aa", -351145363, baseY);
        is.dk = world;
        cq.lq = world;
        xk request = new xk();
        List<Integer> squares = new ArrayList<>();
        for (int x = (baseX - 40) >> 6; x <= (baseX + 143) >> 6; x++)
            for (int y = (baseY - 40) >> 6; y <= (baseY + 143) >> 6; y++)
                if (cache.store.findIndex(5).getArchive((x << 8) | y) != null)
                    squares.add((x << 8) | y);
        request.af = squares.stream().mapToInt(Integer::intValue).toArray();
        world.zn = request.af;
        world.bp = request.ae;
        world.gi = false;
        rl4 loader = new rl4(null, 0, world, request);
        loader.wb = baseX;
        loader.za = baseY;
        loader.xo = baseX / 8 + 6;
        loader.yw = baseY / 8 + 6;
        loader.xn = 0;
        NativeFields.put(null, client.class, "rs", rl4.class, loader);
        if (!loader.fn()) throw new IllegalStateException("Original world loader could not load frozen scenery");
        world.ae = loader.gb;
        NativeFields.put(loader.gb, ez.class, "jd", dz.class, world);
        world.al = loader.nw;
        world.aj = loader.zw;
        scene = loader.gb;
        scene.setDrawDistance(25);
        tileCount = objectCount = 0;
        for (Tile[][] plane : ((Scene) scene).getExtendedTiles())
            for (Tile[] column : plane)
                for (Tile tile : column)
                {
                    if (tile == null) continue;
                    tileCount++;
                    if (tile.getGameObjects() != null)
                        for (GameObject object : tile.getGameObjects()) if (object != null) objectCount++;
                }
        if (tileCount < 10000 || objectCount < 1000)
            throw new IllegalStateException("Incomplete source scene");
        player = null;
        evidence.record("native_scene_loaded", "loader", "unchanged rl4.fn", "renderer", "unchanged ez.dh",
            "base_x", baseX, "base_y", baseY, "tiles", tileCount, "object_tile_references", objectCount,
            "regions", squares, "cache_id", 2695);
    }

    void apply(Game.Player view) throws Exception
    {
        if (view.getTile().getPlane() != 0 || view.hasInstance())
            throw new IllegalStateException("This bounded first-XP adapter supports only the real outdoor plane-zero start route");
        int x = view.getTile().getX(), y = view.getTile().getY();
        if (world == null || x - baseX < 8 || x - baseX > 95 || y - baseY < 8 || y - baseY > 95)
            load(x, y);
        if (player == null)
        {
            player = new ct(0);
            NativeFields.put(player, ct.class, "ae", aae.class, new aae(view.getDisplayName()));
            lc appearance = new lc();
            appearance.ae(null, null, null, false, new int[5],
                view.getAppearanceOrDefault("body_type", 0), penguinId, -1, -1963864182);
            appearance.setTransformedNpcId(penguinId);
            NativeFields.put(player, ct.class, "ab", lc.class, appearance);
            NativeFields.put(null, dc.class, "cb", ct.class, player);
            Object registry = NativeFields.field(client.class, "dr", cl.class).get(null);
            NativeFields.logicalInt(registry, cl.class, "ax", -1688595207, 1);
            NativeFields.put(registry, cl.class, "as", dz.class, world);
            ((yn) NativeFields.field(cl.class, "az", yn.class).get(registry)).af(world, 0L);
            world.aq.af(player, 0L);
            NativeFields.logicalInt(null, client.class, "da", -2034209657, 0);
            NativeFields.logicalInt(null, client.class, "dj", -2130951373, 0);
            byte[] identity = MessageDigest.getInstance("SHA-256")
                .digest(("ClubScape RuneLite actor:" + view.getActorId()).getBytes(StandardCharsets.UTF_8));
            NativeFields.logicalLong(game, client.class, "qa", 9006457894215305539L,
                ByteBuffer.wrap(identity).getLong());
            player.setIdlePoseAnimation(idle);
            player.setWalkAnimation(walk);
            player.setAnimation(-1);
            player.setPoseAnimation(idle);
            if (game.getLocalPlayer() != player || mb.ev(2006617018) != player)
                throw new IllegalStateException("Original local player registry was not populated");
        }
        NativeFields.logicalInt(player, dh.class, "bb", -1547553299, (x - baseX) * 128 + 64);
        NativeFields.logicalInt(player, dh.class, "bi", -1272026483, (y - baseY) * 128 + 64);
        NativeFields.path(player, "mq")[0] = x - baseX;
        NativeFields.path(player, "at")[0] = y - baseY;
        boolean moved = lastPlayer != null && !lastPlayer.getTile().equals(view.getTile());
        player.setPoseAnimation(moved ? walk : idle);
        player.setPoseAnimationFrame(0);
        lastPlayer = view;
        if (player.getWorldLocation().getX() != x || player.getWorldLocation().getY() != y)
            throw new IllegalStateException("Native coordinates differ from authoritative " + x + "," + y
                + ": " + player.getWorldLocation() + " local=" + player.getLocalLocation());
        Model model = ((Player) player).getModel();
        if (model == null || model.getVerticesCount() <= 0 || model.getFaceCount() <= 0)
            throw new IllegalStateException("Original player did not assemble the approved penguin model");
        evidence.record("native_player", "actor_id", view.getActorId(), "x", x, "y", y,
            "plane", view.getTile().getPlane(), "native_x", player.getWorldLocation().getX(),
            "native_y", player.getWorldLocation().getY(), "npc_transform", ((Player) player).getPlayerComposition().getTransformedNpcId(),
            "model_id", 21547, "native_vertices", model.getVerticesCount(), "native_faces", model.getFaceCount(),
            "pose_animation", player.getPoseAnimation(), "source_scale", "75/128",
            "server_animation", view.getAnimation(), "activity", view.getActivity());
    }

    private void draw(boolean includePlayer) throws Exception
    {
        Arrays.fill(provider.getPixels(), 0);
        bindPixels();
        int focalX = player.getLocalLocation().getX(), focalY = player.getLocalLocation().getY();
        int sourceHeight = world.al[0][focalX / 128][focalY / 128];
        int cameraX = focalX, cameraY = focalY - 1280, cameraHeight = sourceHeight - 1000;
        up pitch = new up(2048), yaw = new up(0);
        NativeFields.logicalInt(null, ki.class, "jl", -325062789, cameraX);
        NativeFields.logicalInt(null, nl.class, "jr", 1615527037, cameraHeight);
        NativeFields.logicalInt(null, ai.class, "jo", 1343311673, cameraY);
        NativeFields.put(null, client.class, "jj", up.class, pitch);
        NativeFields.put(null, client.class, "jt", up.class, yaw);
        NativeFields.put(null, client.class, "kb", boolean.class, true);
        if (includePlayer && !scene.bj(0, focalX, focalY, sourceHeight, 60, player, 0, 0L, false))
            throw new IllegalStateException("Original scene refused local player after " + frames
                + " frames at " + focalX + "," + focalY);
        scene.dh(cameraX, cameraHeight, cameraY, pitch, yaw, 0, focalX, focalY, true);
        scene.bw();
    }

    void render() throws Exception
    {
        int w = canvas.getWidth(), h = canvas.getHeight();
        if (w > 0 && h > 0 && (w != width || h != height)) resize(w, h);
        if (player != null)
        {
            int[] lengths = game.loadAnimation(player.getPoseAnimation()).getFrameLengths();
            if (lengths != null && lengths.length > 0)
            {
                int duration = Arrays.stream(lengths).sum();
                int cycle = (int) ((System.nanoTime() / 20_000_000L) % duration);
                int frame = 0;
                while (frame + 1 < lengths.length && cycle >= lengths[frame]) cycle -= lengths[frame++];
                player.setPoseAnimationFrame(frame);
            }
            draw(true);
        }
        Graphics graphics = canvas.getGraphics();
        if (graphics != null)
        {
            try { game.getCallbacks().draw(provider, graphics, 0, 0); }
            finally { graphics.dispose(); }
        }
        frames++;
    }

    long verifyVisibleActor(String classification) throws Exception
    {
        draw(false);
        int[] background = provider.getPixels().clone();
        draw(true);
        long changed = 0;
        for (int i = 0; i < width * height; i++)
            if (background[i] != provider.getPixels()[i]) changed++;
        actorPixels = changed;
        evidence.record("native_actor_visibility", "differing_pixels", changed, "width", width, "height", height,
            "classification", classification,
            "method", "same current native scene/camera, original draw with versus without local player",
            "source_ground_height", world.al[0][player.getLocalLocation().getSceneX()][player.getLocalLocation().getSceneY()],
            "frames", frames);
        return changed;
    }

    BufferedImage image()
    {
        BufferedImage result = new BufferedImage(width, height, BufferedImage.TYPE_INT_RGB);
        result.setRGB(0, 0, width, height, provider.getPixels(), 0, width);
        return result;
    }

    RectangleOnScreen screenRectangle()
    {
        return new RectangleOnScreen(canvas.getLocationOnScreen().x, canvas.getLocationOnScreen().y, width, height);
    }

    record RectangleOnScreen(int x, int y, int width, int height) {}
}
