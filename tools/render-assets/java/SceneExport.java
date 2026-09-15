import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.runelite.api.Scene;
import net.runelite.api.Tile;

/**
 * Builds original scenes with the pinned runtime's map loader (rl4.fn) and serializes the live
 * scene structures (ez) the original software renderer traverses: per-tile flags, heights,
 * tile paints/models, walls, decorations, game objects and the lit models they reference.
 * No candidate rendering happens here.
 */
final class SceneExport
{
    /**
     * Fixture scene loads in the exact order of the original capture (WorldCapture.run). The order
     * matters: animated objects take random start frames from the seeded JVM generator, so each
     * load must see the same generator state as the capture that produced the reference PNG.
     */
    static final Object[][] FIXTURE_SCENES = {
        {"lumbridge-castle-plaza", 3168, 3168},
        {"lumbridge-river-bridge", 3168, 3168},
        {"tutorial-starting-house", 3048, 3056},
        {"tutorial-survival-coast", 3048, 3032},
        {"lumbridge-windmill-route", 3120, 3240},
    };

    final RenderExport export;
    final Map<fx, Integer> modelIndex = new IdentityHashMap<>();
    final List<String> modelKeys = new ArrayList<>();
    final java.util.Set<Integer> usedTextures = new java.util.TreeSet<>();

    SceneExport(RenderExport export) { this.export = export; }

    void run() throws Exception
    {
        // Replay the original capture's preceding model/item/font profile so the seeded random
        // generator is in the same state before the first scene load (fixture profile "all").
        java.lang.reflect.Method modelProfile = OriginalCapture.class.getDeclaredMethod("run");
        modelProfile.setAccessible(true);
        modelProfile.invoke(export.runtime);
        initializeVariables();
        fq.ab(32768);
        ez.cc(25);
        List<Object> scenes = new ArrayList<>();
        for (Object[] entry : FIXTURE_SCENES)
        {
            modelIndex.clear();
            modelKeys.clear();
            contentIndex.clear();
            modelBytes.clear();
            scenes.add(exportScene((String) entry[0], (Integer) entry[1], (Integer) entry[2]));
        }
        export.manifest.put("scenes", scenes);
        export.manifest.put("scene_texture_ids", new ArrayList<>(usedTextures));
        System.out.println("SCENES exported=" + scenes.size() + " textures=" + usedTextures.size());
    }

    private void initializeVariables() throws Exception
    {
        int maximumVarp = 0;
        vp configurations = export.cache.archive(2);
        for (int id : configurations.getFileIds(14))
        {
            net.runelite.api.VarbitComposition definition = new pt(new xy(configurations.loadData(14, id)));
            maximumVarp = Math.max(maximumVarp, definition.getIndex());
        }
        WorldCapture.staticField(lb.class, "af", int[].class, new int[maximumVarp + 1]);
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

    static int rawInt(Object instance, String name) throws Exception
    {
        Field field = instance.getClass().getDeclaredField(name);
        field.setAccessible(true);
        return field.getInt(instance);
    }

    static Object raw(Object instance, Class<?> owner, String name) throws Exception
    {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(instance);
    }

    /** Resolves a Renderable to the lit model the original would draw in this state. */
    /** Result of resolving a renderable: the model and whether it is a shared animation scratch instance. */
    record Resolved(fx model, boolean shared) {}

    Resolved modelOf(ee renderable, List<Object> dynamicNotes) throws Exception
    {
        if (renderable == null) return new Resolved(null, false);
        if (renderable instanceof fx) return new Resolved((fx) renderable, false);
        // Dynamic renderables (animated objects) resolve their current model through the same
        // original ee.ae(int) call the draw path uses (ee.mk).
        java.lang.reflect.Method getter = null;
        for (Class<?> type = renderable.getClass(); type != null && getter == null; type = type.getSuperclass())
        {
            for (java.lang.reflect.Method method : type.getDeclaredMethods())
            {
                if (method.getName().equals("ae") && method.getParameterCount() == 1 && method.getParameterTypes()[0] == int.class
                    && method.getReturnType() == fx.class)
                {
                    getter = method;
                    break;
                }
            }
        }
        if (getter == null) throw new IllegalStateException("Unexpected scene renderable " + renderable.getClass().getName());
        getter.setAccessible(true);
        fx model = (fx) getter.invoke(renderable, -1455788262);
        if (dynamicNotes != null) dynamicNotes.add(renderable.getClass().getName() + (model == null ? "-null" : ""));
        // Animated renderables return a shared scratch model whose contents change per call.
        return new Resolved(model, true);
    }

    final Map<String, Integer> contentIndex = new java.util.HashMap<>();
    final List<byte[]> modelBytes = new ArrayList<>();

    int modelId(Resolved resolved) throws Exception
    {
        fx model = resolved.model();
        if (model == null) return -1;
        if (!resolved.shared())
        {
            Integer existing = modelIndex.get(model);
            if (existing != null) return existing;
        }
        byte[] bytes = RenderExport.model(model).toBytes();
        String sha = ChunkWriter.sha256(bytes);
        Integer byContent = contentIndex.get(sha);
        if (byContent != null)
        {
            if (!resolved.shared()) modelIndex.put(model, byContent);
            return byContent;
        }
        modelBytes.add(bytes);
        if (model.cq != null)
        {
            for (int i = 0; i < model.bd; i++) if (model.cq[i] != -1) usedTextures.add((int) model.cq[i]);
        }
        int id = modelKeys.size();
        modelKeys.add(sha);
        contentIndex.put(sha, id);
        if (!resolved.shared()) modelIndex.put(model, id);
        return id;
    }

    Map<String, Object> exportScene(String name, int baseX, int baseY) throws Exception
    {
        dz world = new dz(0, 104, 104, 25, ex.az);
        WorldCapture.logicalInt(world, dz.class, "ac", -1444178379, baseX);
        WorldCapture.logicalInt(world, dz.class, "aa", -351145363, baseY);
        is.dk = world;
        cq.lq = world;
        xk request = new xk();
        List<Integer> squares = new ArrayList<>();
        for (int x = (baseX - 40) >> 6; x <= (baseX + 143) >> 6; x++)
        {
            for (int y = (baseY - 40) >> 6; y <= (baseY + 143) >> 6; y++)
            {
                int square = x << 8 | y;
                if (export.cache.store.findIndex(5).getArchive(square) != null) squares.add(square);
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
        WorldCapture.staticField(client.class, "rs", rl4.class, loader);
        if (!loader.fn()) throw new IllegalStateException("Original maploader has unavailable source dependencies");
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
        ez ez = loader.gb;
        int tiles = 0, objects = 0;
        for (Tile[][] plane : scene.getExtendedTiles())
        {
            for (Tile[] column : plane)
            {
                for (Tile tile : column)
                {
                    if (tile == null) continue;
                    tiles++;
                    if (tile.getGameObjects() != null) for (var object : tile.getGameObjects()) if (object != null) objects++;
                }
            }
        }
        if (tiles < 10000 || objects < 1000) throw new IllegalStateException("Incomplete native source scenery");

        int planes = rawInt(ez, "bq");
        int width = rawInt(ez, "bf");
        int height = rawInt(ez, "bs");
        int tileCount = rawInt(ez, "ls") + 1;
        int[] flags = ez.xj;
        int[][][] heights = (int[][][]) raw(ez, ez.class, "bd");
        List<Object> dynamicNotes = new ArrayList<>();
        ChunkWriter writer = new ChunkWriter();
        writer.text("NAME", name);
        writer.ints("SCHD", baseX, baseY, width, height, planes, rawInt(ez, "dh"), ez.oy, ez.ok, ez.pe, ez.ne, ez.un,
            ((Boolean) raw(ez, ez.class, "ah")) ? 1 : 0, ez.ny, ez.fb, ez.xu, ez.gf, ez.mh, tileCount, rawInt(ez, "bl"));
        writer.ints("FLAG", flags, tileCount).bytes("LINK", ez.lr, tileCount).bytes("OBJC", ez.jm, tileCount).bytes("OBJF", ez.fm, tileCount * 5);
        int[] heightFlat = new int[planes * (width + 1) * (height + 1)];
        int cursor = 0;
        for (int p = 0; p < planes; p++) for (int x = 0; x <= width; x++) for (int y = 0; y <= height; y++) heightFlat[cursor++] = heights[p][x][y];
        writer.ints("HGHT", heightFlat, heightFlat.length);
        int[] roofFlat = new int[planes * width * height];
        cursor = 0;
        for (int p = 0; p < planes; p++) for (int x = 0; x < width; x++) for (int y = 0; y < height; y++) roofFlat[cursor++] = ez.xc[p][x][y];
        writer.ints("ROOF", roofFlat, roofFlat.length);

        List<Integer> paints = new ArrayList<>();
        List<Integer> tileModels = new ArrayList<>();
        List<Integer> walls = new ArrayList<>();
        List<Integer> wallDecor = new ArrayList<>();
        List<Integer> floorDecor = new ArrayList<>();
        List<Integer> gameObjects = new ArrayList<>();
        int[] counts = new int[6];
        for (int index = 0; index < tileCount; index++)
        {
            // Bridge-moved tiles live at plane 3 without the EXISTS bit; export every slot.
            fj paint = ez.je[index];
            if (paint != null)
            {
                paints.add(index); paints.add(paint.getSwColor()); paints.add(paint.getSeColor()); paints.add(paint.getNeColor());
                paints.add(paint.getNwColor()); paints.add(paint.getTexture()); paints.add(paint.isFlat() ? 1 : 0); paints.add(paint.getRBG());
                if (paint.getTexture() != -1) usedTextures.add(paint.getTexture());
                counts[0]++;
            }
            fn model = ez.kl[index];
            if (model != null)
            {
                int[] vx = model.getVertexX(), vy = model.getVertexY(), vz = model.getVertexZ();
                int[] fa = model.getFaceX(), fb = model.getFaceY(), fc = model.getFaceZ();
                int[] ca = model.getTriangleColorA(), cb = model.getTriangleColorB(), cc = model.getTriangleColorC();
                int[] tex = model.getTriangleTextureId();
                tileModels.add(index); tileModels.add(model.getShape()); tileModels.add(model.getRotation()); tileModels.add(model.isFlat() ? 1 : 0);
                tileModels.add(model.getModelUnderlay()); tileModels.add(model.getModelOverlay()); tileModels.add(vx.length); tileModels.add(fa.length);
                tileModels.add(tex == null ? 0 : 1);
                for (int v : vx) tileModels.add(v);
                for (int v : vy) tileModels.add(v);
                for (int v : vz) tileModels.add(v);
                for (int v : fa) tileModels.add(v);
                for (int v : fb) tileModels.add(v);
                for (int v : fc) tileModels.add(v);
                for (int v : ca) tileModels.add(v);
                for (int v : cb) tileModels.add(v);
                for (int v : cc) tileModels.add(v);
                if (tex != null) for (int v : tex) { tileModels.add(v); if (v != -1) usedTextures.add(v); }
                counts[1]++;
            }
            fe wall = ez.oc[index];
            if (wall != null)
            {
                long hash = wall.getHash();
                walls.add(index); walls.add(modelId(modelOf(wall.af, dynamicNotes))); walls.add(modelId(modelOf(wall.az, dynamicNotes)));
                walls.add(wall.getOrientationA()); walls.add(wall.getOrientationB());
                walls.add(wall.getX()); walls.add(wall.getZ()); walls.add(wall.getY()); walls.add((int) hash); walls.add((int) (hash >>> 32));
                counts[2]++;
            }
            fi decor = ez.vh[index];
            if (decor != null)
            {
                long hash = decor.getHash();
                wallDecor.add(index); wallDecor.add(modelId(modelOf(decor.az, dynamicNotes))); wallDecor.add(modelId(modelOf(decor.af, dynamicNotes)));
                wallDecor.add(decor.ax * 254285683); wallDecor.add(decor.ac * -819410985);
                wallDecor.add(decor.getX()); wallDecor.add(decor.getZ()); wallDecor.add(decor.getY());
                wallDecor.add(decor.getXOffset()); wallDecor.add(decor.getYOffset()); wallDecor.add(decor.getXOffset2()); wallDecor.add(decor.getYOffset2());
                wallDecor.add((int) hash); wallDecor.add((int) (hash >>> 32));
                counts[3]++;
            }
            eo floor = ez.si[index];
            if (floor != null)
            {
                long hash = floor.getHash();
                floorDecor.add(index); floorDecor.add(modelId(modelOf(floor.az, dynamicNotes)));
                floorDecor.add(floor.getX()); floorDecor.add(floor.getZ()); floorDecor.add(floor.getY()); floorDecor.add((int) hash); floorDecor.add((int) (hash >>> 32));
                counts[4]++;
            }
            if (ez.fk[index] != null) throw new IllegalStateException("Unexpected item layer in a static scene export");
            int objectCount = ez.jm[index];
            for (int slot = 0; slot < objectCount; slot++)
            {
                fb object = ez.yx[index * 5 + slot];
                if (object == null) continue;
                long hash = object.getHash();
                gameObjects.add(index); gameObjects.add(slot); gameObjects.add(modelId(modelOf(object.az, dynamicNotes)));
                gameObjects.add(object.getModelOrientation()); gameObjects.add(object.getX()); gameObjects.add(object.getZ()); gameObjects.add(object.getY());
                gameObjects.add(object.getSceneMinLocation().getX()); gameObjects.add(object.getSceneMaxLocation().getX());
                gameObjects.add(object.getSceneMinLocation().getY()); gameObjects.add(object.getSceneMaxLocation().getY());
                gameObjects.add(object.getConfig()); gameObjects.add((int) ez.fm[index * 5 + slot]); gameObjects.add(object.az instanceof fx ? 0 : 1);
                gameObjects.add((int) hash); gameObjects.add((int) (hash >>> 32));
                counts[5]++;
            }
        }
        List<Integer> zoneDynamic = new ArrayList<>();
        for (int zx = 0; zx < (width >> 3); zx++)
        {
            for (int zy = 0; zy < (height >> 3); zy++)
            {
                rl17 zone = ez.ow[zx][zy];
                for (Object entry : zone.hf)
                {
                    if (!(entry instanceof fb)) continue;
                    fb object = (fb) entry;
                    if (object.az instanceof fx) continue;
                    long hash = object.getHash();
                    zoneDynamic.add(zx); zoneDynamic.add(zy); zoneDynamic.add(modelId(modelOf(object.az, dynamicNotes)));
                    zoneDynamic.add(object.getModelOrientation()); zoneDynamic.add(object.getX()); zoneDynamic.add(object.getZ()); zoneDynamic.add(object.getY());
                    zoneDynamic.add(object.getSceneMinLocation().getX()); zoneDynamic.add(object.getSceneMaxLocation().getX());
                    zoneDynamic.add(object.getSceneMinLocation().getY()); zoneDynamic.add(object.getSceneMaxLocation().getY());
                    zoneDynamic.add(object.getConfig()); zoneDynamic.add((int) hash); zoneDynamic.add((int) (hash >>> 32));
                }
            }
        }
        writer.ints("PANT", toArray(paints)).ints("TMOD", toArray(tileModels)).ints("WALL", toArray(walls)).ints("WDEC", toArray(wallDecor));
        writer.ints("FDEC", toArray(floorDecor)).ints("GOBJ", toArray(gameObjects)).ints("ZDYN", toArray(zoneDynamic));
        StringBuilder keys = new StringBuilder();
        for (String key : modelKeys) keys.append(key).append('\n');
        writer.text("MODL", keys.toString());
        String fileKey = "scenes/" + name + ".bin";
        Path file = export.output.resolve(fileKey);
        String sha = writer.write(file);
        // Single-fetch model pack for streaming: every model the scene references, in index order.
        java.io.ByteArrayOutputStream pack = new java.io.ByteArrayOutputStream();
        pack.writeBytes("CSMP".getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        pack.writeBytes(java.nio.ByteBuffer.allocate(4).order(java.nio.ByteOrder.LITTLE_ENDIAN).putInt(modelKeys.size()).array());
        for (int i = 0; i < modelKeys.size(); i++)
        {
            byte[] keyBytes = modelKeys.get(i).getBytes(java.nio.charset.StandardCharsets.US_ASCII);
            byte[] data = modelBytes.get(i);
            pack.writeBytes(java.nio.ByteBuffer.allocate(8).order(java.nio.ByteOrder.LITTLE_ENDIAN).putInt(keyBytes.length).putInt(data.length).array());
            pack.writeBytes(keyBytes);
            pack.writeBytes(data);
        }
        String packKey = "scenes/" + name + ".models.bin";
        byte[] packBytes = pack.toByteArray();
        Files.write(export.output.resolve(packKey), packBytes);
        export.record(packKey, ChunkWriter.sha256(packBytes), packBytes.length, OriginalCapture.map("models", modelKeys.size(), "scene", name));
        Map<String, Object> record = OriginalCapture.map("name", name, "file", fileKey, "sha256", sha, "base_x", baseX, "base_y", baseY,
            "region_ids", squares, "tiles", tiles, "placed_object_tile_references", objects,
            "paints", counts[0], "tile_models", counts[1], "walls", counts[2], "wall_decorations", counts[3], "floor_decorations", counts[4],
            "game_object_slots", counts[5], "distinct_models", modelKeys.size(), "dynamic_renderables", dynamicNotes.size(),
            "source_pipeline", "Original rl4.fn map decoding, placement, floor blending, shadowing; live ez arrays serialized without re-rendering");
        export.record(fileKey, sha, Files.size(file), record);
        System.out.println("SCENE " + name + " tiles=" + tiles + " objects=" + objects + " models=" + modelKeys.size() + " dynamic=" + dynamicNotes.size());
        return record;
    }

    static int[] toArray(List<Integer> values)
    {
        int[] out = new int[values.size()];
        for (int i = 0; i < out.length; i++) out[i] = values.get(i);
        return out;
    }
}
