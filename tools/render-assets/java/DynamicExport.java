import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Exports the lit models for live WorldView layers the static scene exports cannot carry:
 * door states (the original wall model of each door object at every orientation), the fire
 * temporary object (animated: every source frame of its sequence), state variants such as the
 * flour bin, and ground-item models (the original `op.aa` item model per quantity threshold).
 * All models are lit exactly as the original scene/tile item paths light them.
 */
final class DynamicExport
{
    final RenderExport export;

    DynamicExport(RenderExport export) { this.export = export; }

    /** Door object ids used by the M1 content pack's object transforms (wall layer, shape 0). */
    static final int[] DOOR_OBJECTS = {12348, 12349, 12350, 12986, 12987, 13001, 15056, 1521, 1524, 1535, 1540, 1543, 1558, 1560, 23917, 2406,
        56376, 883, 9398, 9470, 9708, 9709, 9710, 9716, 9717, 9718, 9719, 9720, 9721, 9722, 9723};
    /** Fire temporary object and flour-bin state variants (content pack mechanics). */
    static final int[] STATE_OBJECTS = {26185, 5792, 1782, 1781};
    /** All 116 M1 item ids (content pack `items`). */
    static final int[] ITEMS = {288, 289, 303, 304, 315, 316, 317, 318, 436, 437, 438, 439, 526, 527, 550, 551, 555, 556, 557, 558, 559, 590, 591, 592, 593,
        841, 842, 877, 882, 946, 947, 952, 953, 995, 1009, 1010, 1171, 1172, 1173, 1174, 1205, 1206, 1237, 1238, 1265, 1266, 1277, 1278, 1351, 1352,
        1438, 1439, 1511, 1512, 1735, 1736, 1755, 1756, 1887, 1888, 1917, 1918, 1923, 1924, 1925, 1926, 1927, 1928, 1929, 1930, 1931, 1932, 1933,
        1934, 1935, 1936, 1944, 1945, 1947, 1948, 1949, 1950, 2307, 2308, 2309, 2310, 2311, 2312, 2347, 2348, 2349, 2350, 2530, 2531, 3008, 3009,
        3012, 3013, 3014, 3015, 6801, 7954, 7955, 9003, 9014, 10999, 11000, 13447, 13448, 13449, 20742, 20743, 22660, 22661, 33089, 33091};

    void run(String[] args) throws Exception
    {
        new AnimExport(export).bindArchives();
        List<Object> objects = new ArrayList<>();
        for (int id : DOOR_OBJECTS) objects.add(exportObject(id, true));
        for (int id : STATE_OBJECTS) objects.add(exportObject(id, false));
        export.manifest.put("dynamic_objects", objects);
        List<Object> items = new ArrayList<>();
        for (int id : ITEMS) items.add(exportGroundItem(id));
        export.manifest.put("ground_items", items);
        System.out.println("DYNAMIC objects=" + objects.size() + " items=" + items.size());
    }

    /**
     * Lit models of an object for every (type, orientation) its definition supports: doors use
     * wall type 0 (and type 9 when defined); other scenery uses type 10. Animated objects also
     * export every frame of their sequence (`om.sg` with the frame pinned) so a fire can burn.
     */
    Map<String, Object> exportObject(int objectId, boolean door) throws Exception
    {
        om definition = om.ck(objectId);
        if (definition == null) throw new IllegalStateException("Missing object definition " + objectId);
        Method build = om.class.getDeclaredMethod("sg", rl21.class, int.class, int.class, int[][].class, int.class, int.class, int.class, ou.class, int.class);
        build.setAccessible(true);
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("object_id", objectId);
        record.put("name", definition.getName());
        record.put("size_x", definition.getSizeX());
        record.put("size_y", definition.getSizeY());
        // Opcode 24 animation id (this.dc, encoder -129385453).
        int sequenceId = AnimExport.decode(AnimExport.rawInt(definition, om.class, "dc"), -129385453);
        record.put("sequence", sequenceId);
        int[] types = door ? new int[] {0, 9} : new int[] {10, 11, 22};
        List<Object> variants = new ArrayList<>();
        for (int type : types)
        {
            for (int orientation = 0; orientation < 4; orientation++)
            {
                fx model = (fx) build.invoke(definition, rl21.lz, type, orientation, null, 0, 0, 0, null, -1);
                if (model == null) continue;
                String name = "object-" + objectId + "-t" + type + "-r" + orientation;
                if (sequenceId != -1)
                {
                    // Animated: bake each frame with the sequence applied (the plain model first).
                    export.writeModel("dynamic/" + name, model, OriginalCapture.map("object_id", objectId, "type", type, "orientation", orientation, "frame", -1));
                    ou sequence = (ou) export.runtime.game.loadAnimation(sequenceId);
                    if (sequence == null) throw new IllegalStateException("Missing object sequence " + sequenceId);
                    int frames = ((net.runelite.api.Animation) sequence).getNumFrames();
                    List<String> frameFiles = new ArrayList<>();
                    for (int frame = 0; frame < frames; frame++)
                    {
                        fx animated = (fx) build.invoke(definition, rl21.lz, type, orientation, null, 0, 0, 0, sequence, frame);
                        if (animated == null) throw new IllegalStateException("Object " + objectId + " frame " + frame + " has no model");
                        String frameName = name + "-f" + frame;
                        export.writeModel("dynamic/" + frameName, animated, OriginalCapture.map("object_id", objectId, "type", type, "orientation", orientation, "frame", frame));
                        frameFiles.add("models/dynamic/" + frameName + ".bin");
                    }
                    variants.add(OriginalCapture.map("type", type, "orientation", orientation, "model", "models/dynamic/" + name + ".bin",
                        "frames", frameFiles, "frame_lengths_client_cycles", ((net.runelite.api.Animation) sequence).getFrameLengths()));
                }
                else
                {
                    export.writeModel("dynamic/" + name, model, OriginalCapture.map("object_id", objectId, "type", type, "orientation", orientation));
                    variants.add(OriginalCapture.map("type", type, "orientation", orientation, "model", "models/dynamic/" + name + ".bin"));
                }
            }
        }
        record.put("variants", variants);
        System.out.println("DYNOBJ " + objectId + " " + definition.getName() + " variants=" + variants.size() + " seq=" + sequenceId);
        return record;
    }

    /** Ground models of an item for each quantity threshold the definition switches at. */
    Map<String, Object> exportGroundItem(int itemId) throws Exception
    {
        op item = (op) export.runtime.game.getItemDefinition(itemId);
        if (item == null) throw new IllegalStateException("Missing item definition " + itemId);
        Method ground = op.class.getDeclaredMethod("aa", int.class, int.class);
        ground.setAccessible(true);
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("item_id", itemId);
        record.put("name", item.getName());
        // `lj.es` pile selection inputs: shop value (`op.ef`) and the stackable flag (`op.ek == 1`).
        record.put("price", item.getPrice());
        record.put("stackable", item.isStackable());
        List<Object> variants = new ArrayList<>();
        int[] thresholds = thresholds(item);
        int lastModel = Integer.MIN_VALUE;
        for (int quantity : thresholds)
        {
            fx model = (fx) ground.invoke(item, quantity, 557403969);
            if (model == null) continue;
            int identity = System.identityHashCode(model);
            if (identity == lastModel) continue;
            lastModel = identity;
            String name = "item-" + itemId + "-q" + quantity;
            export.writeModel("dynamic/" + name, model, OriginalCapture.map("item_id", itemId, "min_quantity", quantity, "source", "op.aa ground item model, lit 64+ambient/768+contrast (-50,-10,-50)"));
            variants.add(OriginalCapture.map("min_quantity", quantity, "model", "models/dynamic/" + name + ".bin"));
        }
        record.put("variants", variants);
        System.out.println("GROUNDITEM " + itemId + " " + item.getName() + " variants=" + variants.size());
        return record;
    }

    /** Quantity thresholds (`op.eq`) plus 1, ascending and deduplicated. */
    static int[] thresholds(op item) throws Exception
    {
        java.util.TreeSet<Integer> out = new java.util.TreeSet<>();
        out.add(1);
        int[] counts = (int[]) AnimExport.raw(item, op.class, "eq");
        if (counts != null) for (int c : counts) if (c > 0) out.add(c);
        return out.stream().mapToInt(Integer::intValue).toArray();
    }
}
