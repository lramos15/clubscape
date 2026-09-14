import java.lang.reflect.Field;
import java.math.BigInteger;
import java.util.ArrayList;
import java.util.List;
import net.runelite.api.Scene;
import net.runelite.api.Tile;

/** Hosts the original runtime's maploader; no substitute terrain, placement or lighting builder. */
public final class WorldCapture
{
    final OriginalCapture capture;

    WorldCapture(OriginalCapture capture) { this.capture = capture; }

    static void logicalInt(Object instance, Class<?> owner, String name, int decodeMultiplier, int value) throws Exception
    {
        int inverse = BigInteger.valueOf(Integer.toUnsignedLong(decodeMultiplier))
            .modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
        for (Field field : owner.getDeclaredFields())
        {
            if (field.getName().equals(name) && field.getType() == int.class)
            {
                field.setAccessible(true);
                field.setInt(instance, value * inverse);
                return;
            }
        }
        throw new IllegalStateException("Missing exact native int field " + owner.getName() + "." + name);
    }

    static void staticField(Class<?> owner, String name, Class<?> type, Object value) throws Exception
    {
        for (Field field : owner.getDeclaredFields())
        {
            if (field.getName().equals(name) && field.getType() == type)
            {
                field.setAccessible(true);
                field.set(null, value);
                return;
            }
        }
        throw new IllegalStateException("Missing exact native static field " + owner.getName() + "." + name);
    }

    private void initializeVariables() throws Exception
    {
        int maximumVarp = 0;
        vp configurations = capture.cache.archive(2);
        for (int id : configurations.getFileIds(14))
        {
            net.runelite.api.VarbitComposition definition = new pt(new xy(configurations.loadData(14, id)));
            maximumVarp = Math.max(maximumVarp, definition.getIndex());
        }
        staticField(lb.class, "af", int[].class, new int[maximumVarp + 1]);
        System.out.println("CONTROLLED_VARPS all_zero capacity=" + (maximumVarp + 1) + " (not observed account state)");
        for (var method : client.class.getDeclaredMethods())
        {
            if (method.getName().equals("al") && method.getReturnType() == void.class && method.getParameterCount() == 0
                && java.lang.reflect.Modifier.isStatic(method.getModifiers()))
            {
                method.setAccessible(true);
                method.invoke(null);
            }
        }
    }

    void prepareForHud() throws Exception
    {
        initializeVariables();
        fq.ab(32768);
        ez.cc(25);
        view("lumbridge-hud", 3168, 3168, 3222, -1300, 3208, 3222, 3218, 0, false);
    }

    void run() throws Exception
    {
        initializeVariables();
        fq.ab(32768);
        ez.cc(25);
        view("lumbridge-castle-plaza", 3168, 3168, 3222, -1300, 3208, 3222, 3218, 0);
        view("lumbridge-river-bridge", 3168, 3168, 3235, -1300, 3205, 3223, 3217, 2048);
        view("tutorial-starting-house", 3048, 3056, 3094, -1000, 3095, 3094, 3103, 0);
        view("tutorial-survival-coast", 3048, 3032, 3105, -1100, 3076, 3101, 3085, 1024);
        view("lumbridge-windmill-route", 3120, 3240, 3174, -1250, 3295, 3166, 3306, 1536);
    }

    private void view(String name, int baseX, int baseY, int cameraWorldX, int cameraHeightOffset, int cameraWorldY,
                      int focalWorldX, int focalWorldY, int yawInput) throws Exception
    {
        view(name, baseX, baseY, cameraWorldX, cameraHeightOffset, cameraWorldY, focalWorldX, focalWorldY, yawInput, true);
    }

    private void view(String name, int baseX, int baseY, int cameraWorldX, int cameraHeightOffset, int cameraWorldY,
                      int focalWorldX, int focalWorldY, int yawInput, boolean renderCapture) throws Exception
    {
        dz world = new dz(0, 104, 104, 25, ex.az);
        logicalInt(world, dz.class, "ac", -1444178379, baseX);
        logicalInt(world, dz.class, "aa", -351145363, baseY);
        is.dk = world;
        cq.lq = world;
        xk request = new xk();
        List<Integer> squares = new ArrayList<>();
        for (int x = (baseX - 40) >> 6; x <= (baseX + 143) >> 6; x++)
        {
            for (int y = (baseY - 40) >> 6; y <= (baseY + 143) >> 6; y++)
            {
                int square = x << 8 | y;
                if (capture.cache.store.findIndex(5).getArchive(square) != null) squares.add(square);
            }
        }
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
        staticField(client.class, "rs", rl4.class, loader);
        System.out.println("NATIVE_MAPLOADER_BEGIN base=" + baseX + "," + baseY + " squares=" + squares);
        boolean loaded = loader.fn();
        if (!loaded) throw new IllegalStateException("Original maploader has unavailable source dependencies");
        world.ae = loader.gb;
        for (Field field : ez.class.getDeclaredFields())
        {
            if (field.getName().equals("jd") && field.getType() == dz.class)
            {
                field.setAccessible(true);
                field.set(loader.gb, world);
            }
        }
        world.al = loader.nw;
        world.aj = loader.zw;
        Scene scene = loader.gb;
        scene.setDrawDistance(25);
        int tiles = 0, objects = 0;
        for (Tile[][] plane : scene.getExtendedTiles())
        {
            for (Tile[] column : plane)
            {
                for (Tile tile : column)
                {
                    if (tile == null) continue;
                    tiles++;
                    if (tile.getGameObjects() != null)
                    {
                        for (var object : tile.getGameObjects()) if (object != null) objects++;
                    }
                }
            }
        }
        System.out.println("NATIVE_MAPLOADER_OK tiles=" + tiles + " placed_object_tile_references=" + objects
            + " base=" + scene.getBaseX() + "," + scene.getBaseY());
        if (tiles < 10000 || objects < 1000) throw new IllegalStateException("Incomplete native source scenery");
        int[] pixels = capture.target(1920, 1080, 0, 768);
        rl.cu(0, 0, 1920, 1080, true, 317527437);
        int nativeZoom = 0;
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("fk") && field.getType() == int.class)
            {
                field.setAccessible(true);
                nativeZoom = field.getInt(null) * 1129651895;
            }
        }
        if (nativeZoom <= 0) throw new IllegalStateException("Original viewport zoom was not initialized");
        fh.ao.ok(nativeZoom);
        int sourceGroundHeight = world.al[0][focalWorldX - baseX][focalWorldY - baseY];
        int cameraY = sourceGroundHeight + cameraHeightOffset;
        int cameraX = (cameraWorldX - baseX) * 128, cameraZ = (cameraWorldY - baseY) * 128;
        int focalX = (focalWorldX - baseX) * 128, focalY = (focalWorldY - baseY) * 128;
        up pitch = new up(2048), yaw = new up(yawInput);
        logicalInt(null, ki.class, "jl", -325062789, cameraX);
        logicalInt(null, nl.class, "jr", 1615527037, cameraY);
        logicalInt(null, ai.class, "jo", 1343311673, cameraZ);
        staticField(client.class, "jj", up.class, pitch);
        staticField(client.class, "jt", up.class, yaw);
        staticField(client.class, "kb", boolean.class, true);
        if (!renderCapture) return;
        loader.gb.dh(cameraX, cameraY, cameraZ, pitch, yaw, 0, focalX, focalY, true);
        capture.save("scenes/" + name, pixels, 1920, 1080, 0, "original-runtime-scene-fixture",
            OriginalCapture.map("region_ids", squares, "base_x", baseX, "base_y", baseY,
                "source_pipeline", "Original rl4.fn map decoding, placement, floor blending, shadowing and native ez.dh scene rasterization",
                "tiles", tiles, "placed_object_tile_references", objects),
            OriginalCapture.map("camera_local_units", new int[]{cameraX, cameraY, cameraZ},
                "source_focal_ground_height", sourceGroundHeight, "camera_height_offset_from_ground", cameraHeightOffset,
                "camera_world_tile_xz", new int[]{cameraWorldX, cameraWorldY}, "focal_world_tile", new int[]{focalWorldX, focalWorldY},
                "pitch_input", 2048, "yaw_input", yawInput, "angle_units_per_turn", 16384,
                "native_camera_readback", OriginalCapture.map("Client.getCameraX", capture.game.getCameraX(),
                    "Client.getCameraY", capture.game.getCameraY(), "Client.getCameraZ", capture.game.getCameraZ(),
                    "Client.getCameraPitch", capture.game.getCameraPitch(), "Client.getCameraYaw", capture.game.getCameraYaw()),
                "camera_mode", "Original locked-camera path; no invented observed player camera",
                "zoom", nativeZoom, "draw_distance", 25,
                "viewport_setup", "Original rl.cu(0,0,1920,1080,true,317527437), including original visibility calculations",
                "far_clip_units", 32768,
                "far_clip_note", "Explicit fixture projection setting via original fq.ab; not an observed live-session default. Native visibility and occlusion stay enabled.",
                "draw_plane_argument", 0, "varps", "all zero, explicitly controlled and not observed account state",
                "server_entity_layers", "No player, NPC spawn table or dropped items fabricated; this is original cache terrain/scenery.",
                "ui", "Viewport only; not a full Resizable-Classic HUD or legitimate source journey capture"), 20000);
    }
}
