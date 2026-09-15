import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;

/**
 * Exports the original minimap inputs the renderer's {@code scene::minimap} port of
 * {@code client.bm / pv / ed / nq / xx / ga} needs beyond the published world blocks:
 *
 * <ul>
 * <li>{@code minimap/mapscenes.bin}: the original map-scene indexed sprites ({@code oy.aq},
 * loaded through {@code hk.ao} from the sprite group named by the graphics defaults) and the 16
 * tile-shape coverage masks {@code client.gq()} rasterises from the original tile-shape models;
 * </li>
 * <li>{@code minimap/blocks/<square>.bin}: per wall the original placement config
 * ({@code fe.getConfig()}: type | rotation << 6), which the published block records omit (types
 * 1 and 3 share the same wall orientation), and for every object definition referenced by the
 * square's walls, game objects and floor decorations its {@code mapSceneId}, size and map icon.
 * </li>
 * </ul>
 *
 * Nothing here draws the minimap; the exact source raster is produced by the Rust port from
 * these buffers and the already published tile/wall/object records.
 */
final class MinimapExport
{
    final RenderExport export;
    final SceneExport scenes;

    MinimapExport(RenderExport export)
    {
        this.export = export;
        this.scenes = new SceneExport(export);
    }

    void run(String[] args) throws Exception
    {
        List<Integer> squares = new ArrayList<>();
        for (int i = 3; i < args.length; i++) squares.add(Integer.parseInt(args[i]));
        if (squares.isEmpty()) throw new IllegalArgumentException("minimap profile needs map square ids");
        Method modelProfile = OriginalCapture.class.getDeclaredMethod("run");
        modelProfile.setAccessible(true);
        modelProfile.invoke(export.runtime);
        scenes.initializeVariables();
        fq.ab(32768);
        ez.cc(25);
        exportMapScenes();
        List<Object> records = new ArrayList<>();
        for (int square : squares)
        {
            if (export.cache.store.findIndex(5).getArchive(square) == null)
            {
                throw new IllegalStateException("Map square " + square + " is not in the verified source cache");
            }
            records.add(exportBlock(square));
        }
        export.manifest.put("minimap_blocks", records);
        System.out.println("MINIMAP blocks=" + records.size());
    }

    /** Original map-scene sprites and the tile-shape masks, straight from the original loaders. */
    void exportMapScenes() throws Exception
    {
        // Graphics defaults (archive 17) name the map-scene sprite group, as in client init.
        aam defaults = new aam();
        defaults.az(export.cache.archive(17), -243617527);
        int spriteGroup = defaults.ap * -876542085;
        if (!hk.ao(export.cache.archive(8), spriteGroup, 0, (byte) -113))
        {
            throw new IllegalStateException("Original map-scene sprite group " + spriteGroup + " unavailable");
        }
        yz[] sprites = fs.ac((byte) 10);
        long[] shapes = client.gq();
        if (shapes.length != 16 || shapes[0] != 0L || shapes[1] != -1L)
        {
            throw new IllegalStateException("Original tile-shape masks did not rasterise as the client asserts");
        }
        ChunkWriter writer = new ChunkWriter();
        writer.text("NAME", "mapscenes");
        writer.ints("MSHD", sprites.length, spriteGroup, sprites.length == 0 ? 0 : sprites[0].af.length);
        writer.longs("MSHP", shapes);
        List<Integer> header = new ArrayList<>();
        java.io.ByteArrayOutputStream pixels = new java.io.ByteArrayOutputStream();
        int[] palette = null;
        for (yz sprite : sprites)
        {
            if (palette == null) palette = sprite.af;
            else if (palette != sprite.af) throw new IllegalStateException("Map-scene sprites do not share one palette");
            header.add(sprite.ax); header.add(sprite.ac); header.add(sprite.ag); header.add(sprite.as);
            header.add(sprite.ae); header.add(sprite.ab); header.add(sprite.az.length);
            pixels.writeBytes(sprite.az);
        }
        writer.ints("MSPR", SceneExport.toArray(header));
        writer.ints("MSPL", palette == null ? new int[0] : palette);
        byte[] indices = pixels.toByteArray();
        writer.bytes("MSPX", indices, indices.length);
        String fileKey = "minimap/mapscenes.bin";
        Path file = export.output.resolve(fileKey);
        Files.createDirectories(file.getParent());
        String sha = writer.write(file);
        export.record(fileKey, sha, Files.size(file), OriginalCapture.map("sprites", sprites.length, "sprite_group", spriteGroup,
            "shape_masks", 16, "source", "oy.aq via hk.ao/fs.ac; client.gq() tile-shape rasterisation"));
        System.out.println("MAPSCENES sprites=" + sprites.length + " group=" + spriteGroup);
    }

    Map<String, Object> exportBlock(int square) throws Exception
    {
        int rx = square >> 8, ry = square & 0xFF;
        int originX = rx * BlockExport.SIZE, originY = ry * BlockExport.SIZE;
        int baseX = originX - BlockExport.MARGIN, baseY = originY - BlockExport.MARGIN;
        ez ez = scenes.loadScene(baseX, baseY);
        int planes = SceneExport.rawInt(ez, "bq");
        int oy = ez.oy;
        int yBits = ez.fb, planeShift = ez.xu;
        List<Integer> walls = new ArrayList<>();
        TreeMap<Integer, om> definitions = new TreeMap<>();
        for (int p = 0; p < planes; p++)
        {
            for (int bx = 0; bx < BlockExport.SIZE; bx++)
            {
                for (int by = 0; by < BlockExport.SIZE; by++)
                {
                    int ex = bx + BlockExport.MARGIN + oy, ey = by + BlockExport.MARGIN + oy;
                    int index = (p << planeShift) | (ex << yBits) | ey;
                    fe wall = ez.oc[index];
                    if (wall != null)
                    {
                        walls.add(p); walls.add(bx); walls.add(by); walls.add(wall.getConfig());
                        definition(definitions, wall.getHash());
                    }
                    eo floor = ez.si[index];
                    if (floor != null) definition(definitions, floor.getHash());
                    int slots = ez.jm[index];
                    for (int slot = 0; slot < slots; slot++)
                    {
                        fb object = ez.yx[index * 5 + slot];
                        if (object != null) definition(definitions, object.getHash());
                    }
                }
            }
        }
        List<Integer> defs = new ArrayList<>();
        for (Map.Entry<Integer, om> entry : definitions.entrySet())
        {
            om def = entry.getValue();
            defs.add(entry.getKey()); defs.add(def.getMapSceneId()); defs.add(def.zf()); defs.add(def.ib()); defs.add(def.getMapIconId());
        }
        ChunkWriter writer = new ChunkWriter();
        writer.text("NAME", "minimap-" + square);
        writer.ints("MBHD", square, rx, ry, originX, originY, BlockExport.SIZE, planes, walls.size() / 4, definitions.size());
        writer.ints("MWAL", SceneExport.toArray(walls));
        writer.ints("MDEF", SceneExport.toArray(defs));
        String fileKey = "minimap/blocks/" + square + ".bin";
        Path file = export.output.resolve(fileKey);
        Files.createDirectories(file.getParent());
        String sha = writer.write(file);
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("square", square);
        record.put("file", fileKey);
        record.put("sha256", sha);
        record.put("walls", walls.size() / 4);
        record.put("object_definitions", definitions.size());
        record.put("source_pipeline", "Original rl4.fn scene load (same base as the block export); fe.getConfig per wall; om mapSceneId/size/mapIcon per referenced object id");
        export.record(fileKey, sha, Files.size(file), record);
        System.out.println("MINIMAP " + square + " walls=" + walls.size() / 4 + " definitions=" + definitions.size());
        return record;
    }

    /** Object id from the original tag (bits 20..51) and its definition, loaded once. */
    static void definition(Map<Integer, om> definitions, long tag) throws Exception
    {
        int id = (int) (tag >>> 20 & 0xFFFFFFFFL);
        if (definitions.containsKey(id)) return;
        om def = om.ck(id);
        if (def == null) throw new IllegalStateException("Missing object definition " + id);
        definitions.put(id, def);
    }
}
