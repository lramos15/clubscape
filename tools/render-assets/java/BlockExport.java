import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Exports the original world as 64x64 map-square blocks the renderer assembles into 104x104
 * scenes around any base (what the original client does when it rebuilds the scene near a
 * region edge). Each square is loaded through the original map loader with at least 16 tiles of
 * real neighbour margin on every side (far beyond the 5-tile floor blend radius), so floor
 * blending, shading and ground contouring inside the square are exactly what any scene
 * containing it would produce. Animated scenery is baked as
 * every source frame of its sequence plus the original sequence timing.
 */
final class BlockExport
{
    /** Low-side margin; the original loader requires bases on the 8-tile chunk lattice. */
    static final int MARGIN = 24;
    static final int SIZE = 64;

    final RenderExport export;
    final SceneExport scenes;
    final java.util.Set<Integer> usedTextures = new java.util.TreeSet<>();

    BlockExport(RenderExport export)
    {
        this.export = export;
        this.scenes = new SceneExport(export);
    }

    void run(String[] args) throws Exception
    {
        List<Integer> squares = new ArrayList<>();
        for (int i = 3; i < args.length; i++) squares.add(Integer.parseInt(args[i]));
        if (squares.isEmpty()) throw new IllegalArgumentException("blocks profile needs map square ids");
        // Same bootstrap as the fixture scenes: the capture's model profile initialises the
        // original render target/state the map loader asserts on.
        Method modelProfile = OriginalCapture.class.getDeclaredMethod("run");
        modelProfile.setAccessible(true);
        modelProfile.invoke(export.runtime);
        scenes.initializeVariables();
        fq.ab(32768);
        ez.cc(25);
        List<Object> records = new ArrayList<>();
        for (int square : squares)
        {
            if (export.cache.store.findIndex(5).getArchive(square) == null)
            {
                throw new IllegalStateException("Map square " + square + " is not in the verified source cache");
            }
            scenes.modelIndex.clear();
            scenes.modelKeys.clear();
            scenes.contentIndex.clear();
            scenes.shapeIndex.clear();
            scenes.modelBytes.clear();
            records.add(exportBlock(square));
        }
        usedTextures.addAll(scenes.usedTextures);
        export.manifest.put("blocks", records);
        export.manifest.put("block_texture_ids", new ArrayList<>(usedTextures));
        System.out.println("BLOCKS exported=" + records.size() + " textures=" + usedTextures.size());
    }

    /** Animated renderable state: frames baked one by one through the original model builder. */
    record DynamicSet(int index, int frames, int[] lengths, int frameStep, int maxLoops, int elapsedModulo, int[] models, int plain) {}

    final Map<Object, Integer> dynamicIndex = new java.util.IdentityHashMap<>();
    final List<DynamicSet> dynamicSets = new ArrayList<>();

    /**
     * Model reference: >= 0 static model index, -1 none, <= -2 animated set `-(ref) - 2` whose
     * frame models are listed in the block's DYNA table.
     */
    int modelRef(ee renderable) throws Exception
    {
        if (renderable == null) return -1;
        if (renderable instanceof fx) return scenes.modelId(scenes.modelOf(renderable, null));
        if (!(renderable instanceof dy)) throw new IllegalStateException("Unexpected scene renderable " + renderable.getClass().getName());
        Integer existing = dynamicIndex.get(renderable);
        if (existing != null)
        {
            if (existing == -1) return -1;
            if (existing < -1) return existing - Integer.MIN_VALUE;
            return -(existing) - 2;
        }
        dy dynamic = (dy) renderable;
        qr state = (qr) SceneExport.raw(dynamic, dy.class, "ac");
        ou sequence = (ou) SceneExport.raw(state, qr.class, "ae");
        Method build = dy.class.getDeclaredMethod("vn", rl21.class, qr.class);
        build.setAccessible(true);
        if (sequence == null)
        {
            // Dynamic renderable without a sequence: the original resolves its plain model.
            fx model = (fx) build.invoke(dynamic, rl21.lz, state);
            int id = model == null ? -1 : scenes.modelId(new SceneExport.Resolved(model, true));
            dynamicIndex.put(renderable, id < 0 ? -1 : Integer.MIN_VALUE + id);
            return id;
        }
        int[] frameIds = (int[]) SceneExport.raw(sequence, ou.class, "bg");
        int[] lengths = ((net.runelite.api.Animation) sequence).getFrameLengths();
        int frameStep = SceneExport.rawInt(sequence, "bu") * 1665914959;
        int maxLoops = SceneExport.rawInt(sequence, "bf") * 2035920365;
        int elapsedModulo = SceneExport.rawInt(sequence, "bo") * -826664243;
        Method pin = qr.class.getDeclaredMethod("ct", int.class, int.class, int.class);
        pin.setAccessible(true);
        int[] models = new int[frameIds.length];
        for (int frame = 0; frame < frameIds.length; frame++)
        {
            pin.invoke(state, frame, 0, 0);
            fx model = (fx) build.invoke(dynamic, rl21.lz, state);
            if (model == null)
            {
                // Morphing scenery without a model in the exported varp state: the original draw
                // path also resolves it to nothing.
                pin.invoke(state, 0, 0, 0);
                dynamicIndex.put(renderable, -1);
                return -1;
            }
            models[frame] = scenes.modelId(new SceneExport.Resolved(model, true));
        }
        pin.invoke(state, 0, 0, 0);
        // The plain (unanimated) model the original shows once a one-shot sequence has ended
        // (`qr.iw` clears the sequence): build it with the sequence detached.
        java.lang.reflect.Field sequenceField = qr.class.getDeclaredField("ae");
        sequenceField.setAccessible(true);
        sequenceField.set(state, null);
        fx plainModel = (fx) build.invoke(dynamic, rl21.lz, state);
        sequenceField.set(state, sequence);
        int plain = plainModel == null ? -1 : scenes.modelId(new SceneExport.Resolved(plainModel, true));
        int index = dynamicSets.size();
        dynamicSets.add(new DynamicSet(index, frameIds.length, lengths, frameStep, maxLoops, elapsedModulo, models, plain));
        dynamicIndex.put(renderable, index);
        return -index - 2;
    }

    Map<String, Object> exportBlock(int square) throws Exception
    {
        int rx = square >> 8, ry = square & 0xFF;
        int originX = rx * SIZE, originY = ry * SIZE;
        int baseX = originX - MARGIN, baseY = originY - MARGIN;
        ez ez = scenes.loadScene(baseX, baseY);
        dynamicIndex.clear();
        dynamicSets.clear();
        int planes = SceneExport.rawInt(ez, "bq");
        int width = SceneExport.rawInt(ez, "bf");
        int oy = ez.oy;
        int yBits = ez.fb, planeShift = ez.xu;
        int[][][] heights = (int[][][]) SceneExport.raw(ez, ez.class, "bd");
        int[] flags = new int[planes * SIZE * SIZE];
        byte[] link = new byte[planes * SIZE * SIZE];
        byte[] objectCount = new byte[planes * SIZE * SIZE];
        byte[] objectFlags = new byte[planes * SIZE * SIZE * 5];
        int[] heightFlat = new int[planes * (SIZE + 1) * (SIZE + 1)];
        int[] roofFlat = new int[planes * SIZE * SIZE];
        byte[] settingsFlat = new byte[planes * SIZE * SIZE];
        List<Integer> paints = new ArrayList<>();
        List<Integer> tileModels = new ArrayList<>();
        List<Integer> walls = new ArrayList<>();
        List<Integer> wallDecor = new ArrayList<>();
        List<Integer> floorDecor = new ArrayList<>();
        List<Integer> gameObjects = new ArrayList<>();
        int unitShift = MARGIN * 128;
        int[] counts = new int[6];
        for (int p = 0; p < planes; p++)
        {
            for (int bx = 0; bx <= SIZE; bx++)
            {
                for (int by = 0; by <= SIZE; by++)
                {
                    heightFlat[(p * (SIZE + 1) + bx) * (SIZE + 1) + by] = heights[p][bx + MARGIN + oy][by + MARGIN + oy];
                }
            }
            for (int bx = 0; bx < SIZE; bx++)
            {
                for (int by = 0; by < SIZE; by++)
                {
                    int ex = bx + MARGIN + oy, ey = by + MARGIN + oy;
                    int index = (p << planeShift) | (ex << yBits) | ey;
                    int local = (p * SIZE + bx) * SIZE + by;
                    flags[local] = ez.xj[index];
                    link[local] = ez.lr[index];
                    objectCount[local] = ez.jm[index];
                    System.arraycopy(ez.fm, index * 5, objectFlags, local * 5, 5);
                    roofFlat[local] = ez.xc[p][ex][ey];
                    settingsFlat[local] = ez.vs[p][ex][ey];
                    fj paint = ez.je[index];
                    if (paint != null)
                    {
                        paints.add(p); paints.add(bx); paints.add(by);
                        paints.add(paint.getSwColor()); paints.add(paint.getSeColor()); paints.add(paint.getNeColor()); paints.add(paint.getNwColor());
                        paints.add(paint.getTexture()); paints.add(paint.isFlat() ? 1 : 0); paints.add(paint.getRBG());
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
                        tileModels.add(p); tileModels.add(bx); tileModels.add(by);
                        tileModels.add(model.getShape()); tileModels.add(model.getRotation()); tileModels.add(model.isFlat() ? 1 : 0);
                        tileModels.add(model.getModelUnderlay()); tileModels.add(model.getModelOverlay()); tileModels.add(vx.length); tileModels.add(fa.length);
                        tileModels.add(tex == null ? 0 : 1);
                        // Tile model vertices are scene-local units; shift them to square-relative units.
                        for (int v : vx) tileModels.add(v - unitShift);
                        for (int v : vy) tileModels.add(v);
                        for (int v : vz) tileModels.add(v - unitShift);
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
                        walls.add(p); walls.add(bx); walls.add(by);
                        walls.add(modelRef(wall.af)); walls.add(modelRef(wall.az));
                        walls.add(wall.getOrientationA()); walls.add(wall.getOrientationB());
                        walls.add(wall.getX() - unitShift); walls.add(wall.getZ()); walls.add(wall.getY() - unitShift);
                        walls.add((int) hash); walls.add((int) (hash >>> 32));
                        counts[2]++;
                    }
                    fi decor = ez.vh[index];
                    if (decor != null)
                    {
                        long hash = decor.getHash();
                        wallDecor.add(p); wallDecor.add(bx); wallDecor.add(by);
                        wallDecor.add(modelRef(decor.az)); wallDecor.add(modelRef(decor.af));
                        wallDecor.add(decor.ax * 254285683); wallDecor.add(decor.ac * -819410985);
                        wallDecor.add(decor.getX() - unitShift); wallDecor.add(decor.getZ()); wallDecor.add(decor.getY() - unitShift);
                        wallDecor.add(decor.getXOffset()); wallDecor.add(decor.getYOffset()); wallDecor.add(decor.getXOffset2()); wallDecor.add(decor.getYOffset2());
                        wallDecor.add((int) hash); wallDecor.add((int) (hash >>> 32));
                        counts[3]++;
                    }
                    eo floor = ez.si[index];
                    if (floor != null)
                    {
                        long hash = floor.getHash();
                        floorDecor.add(p); floorDecor.add(bx); floorDecor.add(by);
                        floorDecor.add(modelRef(floor.az));
                        floorDecor.add(floor.getX() - unitShift); floorDecor.add(floor.getZ()); floorDecor.add(floor.getY() - unitShift);
                        floorDecor.add((int) hash); floorDecor.add((int) (hash >>> 32));
                        counts[4]++;
                    }
                    if (ez.fk[index] != null) throw new IllegalStateException("Unexpected item layer in a static block export");
                    int slots = ez.jm[index];
                    for (int slot = 0; slot < slots; slot++)
                    {
                        fb object = ez.yx[index * 5 + slot];
                        if (object == null) continue;
                        long hash = object.getHash();
                        gameObjects.add(p); gameObjects.add(bx); gameObjects.add(by); gameObjects.add(slot);
                        gameObjects.add(modelRef(object.az));
                        gameObjects.add(object.getModelOrientation());
                        gameObjects.add(object.getX() - unitShift); gameObjects.add(object.getZ()); gameObjects.add(object.getY() - unitShift);
                        // Tile spans are main-area coordinates; store them relative to the square origin.
                        gameObjects.add(object.getSceneMinLocation().getX() - MARGIN); gameObjects.add(object.getSceneMaxLocation().getX() - MARGIN);
                        gameObjects.add(object.getSceneMinLocation().getY() - MARGIN); gameObjects.add(object.getSceneMaxLocation().getY() - MARGIN);
                        gameObjects.add(object.getConfig()); gameObjects.add((int) ez.fm[index * 5 + slot]);
                        gameObjects.add((int) hash); gameObjects.add((int) (hash >>> 32));
                        counts[5]++;
                    }
                }
            }
        }
        List<Integer> dynamic = new ArrayList<>();
        for (DynamicSet set : dynamicSets)
        {
            dynamic.add(set.frames()); dynamic.add(set.frameStep()); dynamic.add(set.maxLoops()); dynamic.add(set.elapsedModulo()); dynamic.add(set.plain());
            for (int i = 0; i < set.frames(); i++) dynamic.add(set.models()[i]);
            for (int i = 0; i < set.frames(); i++) dynamic.add(set.lengths()[i]);
        }
        ChunkWriter writer = new ChunkWriter();
        writer.text("NAME", "block-" + square);
        writer.ints("BLHD", square, rx, ry, originX, originY, SIZE, planes, baseX, baseY, MARGIN, ez.ny, SceneExport.rawInt(ez, "bl"), dynamicSets.size());
        writer.ints("BFLG", flags, flags.length).bytes("BLNK", link, link.length).bytes("BOBC", objectCount, objectCount.length).bytes("BOBF", objectFlags, objectFlags.length);
        writer.ints("BHGT", heightFlat, heightFlat.length).ints("BROF", roofFlat, roofFlat.length).bytes("BSET", settingsFlat, settingsFlat.length);
        writer.ints("BPNT", SceneExport.toArray(paints)).ints("BTMD", SceneExport.toArray(tileModels)).ints("BWAL", SceneExport.toArray(walls));
        writer.ints("BWDC", SceneExport.toArray(wallDecor)).ints("BFDC", SceneExport.toArray(floorDecor)).ints("BOBJ", SceneExport.toArray(gameObjects));
        writer.ints("BDYN", SceneExport.toArray(dynamic));
        StringBuilder keys = new StringBuilder();
        for (String key : scenes.modelKeys) keys.append(key).append('\n');
        writer.text("MODL", keys.toString());
        String fileKey = "blocks/" + square + ".bin";
        Path file = export.output.resolve(fileKey);
        Files.createDirectories(file.getParent());
        String sha = writer.write(file);
        String packKey = "blocks/" + square + ".models.bin";
        byte[] packBytes = SceneExport.pack(scenes.modelKeys, scenes.modelBytes);
        Files.write(export.output.resolve(packKey), packBytes);
        String packSha = ChunkWriter.sha256(packBytes);
        export.record(packKey, packSha, packBytes.length, OriginalCapture.map("models", scenes.modelKeys.size(), "block", square));
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("square", square);
        record.put("file", fileKey);
        record.put("sha256", sha);
        record.put("models_file", packKey);
        record.put("models_sha256", packSha);
        record.put("origin_x", originX);
        record.put("origin_y", originY);
        record.put("size", SIZE);
        record.put("export_base", List.of(baseX, baseY));
        record.put("paints", counts[0]);
        record.put("tile_models", counts[1]);
        record.put("walls", counts[2]);
        record.put("wall_decorations", counts[3]);
        record.put("floor_decorations", counts[4]);
        record.put("game_object_slots", counts[5]);
        record.put("distinct_models", scenes.modelKeys.size());
        record.put("animated_sets", dynamicSets.size());
        record.put("source_pipeline", "Original rl4.fn map decoding with >=16-tile real neighbour margin; square serialized; animated scenery baked per source frame");
        export.record(fileKey, sha, Files.size(file), record);
        System.out.println("BLOCK " + square + " paints=" + counts[0] + " objects=" + counts[5] + " models=" + scenes.modelKeys.size() + " animated=" + dynamicSets.size());
        return record;
    }
}
