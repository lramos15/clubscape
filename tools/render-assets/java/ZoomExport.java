import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * `--profile zoom`: the original client's own full-HUD viewport zoom (`client.fk` after the
 * Resizable-Classic root 161 layout ran through `cn.ae` for a canvas size) sampled across the
 * approved window range, plus the zoom parameters the layout scripts install (`fy/fg` hop
 * values from opcode 6200, `fu/fz/fh/fq` limits from 6202). The renderer adapter ports
 * `client.oh` with these parameters; the recorded native table pins that port.
 */
final class ZoomExport
{
    final RenderExport export;

    ZoomExport(RenderExport export) { this.export = export; }

    static final int[][] SIZES = {
        {1024, 768}, {1280, 720}, {1280, 800}, {1366, 768}, {1440, 900}, {1536, 864}, {1600, 900},
        {1680, 1050}, {1920, 1080}, {1920, 1200}, {2048, 1152}, {2560, 1440}, {1280, 1024}, {1024, 1024},
        {2560, 1080}, {1100, 700},
    };

    static Object field(Class<?> owner, String name, Class<?> type) throws Exception
    {
        for (Field field : owner.getDeclaredFields())
        {
            if (field.getName().equals(name) && field.getType() == type)
            {
                field.setAccessible(true);
                return field.get(null);
            }
        }
        throw new IllegalStateException("Missing exact native field " + owner + "." + name);
    }

    static Object call(Object target, Class<?> owner, String name, Class<?>[] types, Object... values) throws Exception
    {
        Method method = owner.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return method.invoke(target, values);
    }

    int zoom() throws Exception { return (Integer) field(client.class, "fk", int.class) * 1129651895; }
    int viewportWidth() throws Exception { return (Integer) field(client.class, "fv", int.class) * 27064125; }
    int viewportHeight() throws Exception { return (Integer) field(client.class, "fn", int.class) * 1158148203; }
    int shortField(String name) throws Exception { return (Short) field(client.class, name, short.class); }

    Map<String, Object> parameters() throws Exception
    {
        return OriginalCapture.map("fy", shortField("fy"), "fg", shortField("fg"), "fu", shortField("fu"), "fz", shortField("fz"),
            "fh", shortField("fh"), "fq", shortField("fq"), "fi", shortField("fi"), "fb", shortField("fb"));
    }

    void run(String[] args) throws Exception
    {
        OriginalCapture runtime = export.runtime;
        HudCapture hud = new HudCapture(runtime);
        int[] questTotals = HudSourceCatalog.write(runtime.cache, runtime.output);
        new WorldCapture(runtime).prepareForHud();
        client.gc(-1);
        WorldCapture.staticField(lb.class, "az", int[].class, runtime.game.getVarps().clone());
        call(hud, HudCapture.class, "player", new Class<?>[]{});
        call(hud, HudCapture.class, "inventory", new Class<?>[]{});
        for (int[] varbit : new int[][]{{4609, 1}, {5605, 1}, {8119, 1}, {357, 17}, {11877, questTotals[0]}, {1782, questTotals[1]}, {6347, 0}})
            runtime.game.setVarbit(varbit[0], varbit[1]);
        WorldCapture.logicalInt(null, ba.class, "ao", -782895767, 0);
        WorldCapture.logicalInt(null, client.class, "np", 2106329293, (3222 - 3168) * 128 + 64);
        WorldCapture.logicalInt(null, client.class, "nq", -2126074583, (3218 - 3168) * 128 + 64);
        call(hud, HudCapture.class, "minimap", new Class<?>[]{});
        Map<String, Object> defaults = parameters();
        List<Object> samples = new ArrayList<>();
        Map<String, Object> afterLayout = null;
        for (int[] size : SIZES)
        {
            int width = size[0], height = size[1];
            runtime.target(width, height, 0, 512);
            // The client's own canvas size statics (`sa.qy` / `eu.qx`), which the layout scripts
            // read and `client.in`/`acr` pass to `cn.ae` on a real resize.
            WorldCapture.logicalInt(null, sa.class, "qy", 773246731, width);
            WorldCapture.logicalInt(null, eu.class, "qx", 8379747, height);
            call(hud, HudCapture.class, "load", new Class<?>[]{int.class, boolean.class}, 161, false);
            qn context = (qn) field(client.class, "ca", qn.class);
            if (context == null) throw new IllegalStateException("Missing original widget context");
            cn.ae(161, width, height, false, hud.widgets, context, (short) 217);
            call(hud, HudCapture.class, "load", new Class<?>[]{int.class, boolean.class}, 161, true);
            Map<String, Object> parameters = parameters();
            if (afterLayout == null) afterLayout = parameters;
            else if (!afterLayout.toString().equals(parameters.toString()))
                throw new IllegalStateException("Layout zoom parameters changed between sizes: " + afterLayout + " vs " + parameters);
            int zoom = zoom();
            System.out.println("NATIVE_ZOOM " + width + "x" + height + " viewport=" + viewportWidth() + "x" + viewportHeight() + " zoom=" + zoom);
            if (viewportWidth() != width || viewportHeight() != height)
                throw new IllegalStateException("Resizable-Classic viewport did not follow the canvas: " + viewportWidth() + "x" + viewportHeight());
            samples.add(OriginalCapture.map("canvas", new int[]{width, height},
                "viewport", new int[]{viewportWidth(), viewportHeight()}, "zoom", zoom));
            if (width == 1920 && height == 1080 && zoom != 410)
                throw new IllegalStateException("Frozen full-HUD projection changed: zoom=" + zoom + " at 1920x1080 (expected 410)");
        }
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("schema_version", 1);
        record.put("layout", "Original Resizable-Classic root 161 (client.oh after cn.ae)");
        record.put("zoom_formula", "client.oh(x, y, width, height): hop = fy below 334px, fg from 434px, interpolated between; "
            + "512*hop*height/(width*334) clamped by fh..fq (recomputing hop from width, capped fz/fu); zoom = hop*height/334");
        record.put("defaults_before_layout", defaults);
        record.put("parameters_after_layout", afterLayout);
        record.put("samples", samples);
        record.put("viewport_only_fixture_zoom", 662);
        record.put("classification", "Native projection parameters read from the original runtime; not a candidate render");
        byte[] json = (OriginalCapture.JSON.toJson(record) + "\n").getBytes();
        Files.createDirectories(export.output.resolve("hud"));
        Files.write(export.output.resolve("hud/zoom-table.json"), json);
        export.record("hud/zoom-table.json", OriginalCapture.hash(json), json.length, OriginalCapture.map("samples", samples.size()));
        export.manifest.put("hud_zoom", OriginalCapture.map("file", "hud/zoom-table.json", "samples", samples.size(),
            "full_hud_zoom_1920x1080", 410, "viewport_only_fixture_zoom", 662));
    }
}
