import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Records the original if3 model-widget parameters the renderer needs to reproduce interface
 * model draws (the character-creator player preview, interface 679 component 73): the client
 * projects those models with {@code fx.be(0, rotationZ, rotationY, rotationX, offsetX2,
 * sin[rotationX] * modelZoom >> 16 + offsetY2, cos[rotationX] * modelZoom >> 16 + offsetY2)}
 * centred on the component (gp.java type-6 branch); content type 328 additionally forces
 * rotationX 150, rotationZ (int)(sin(cycle / 40.0) * 256) & 2047 and the local player's own
 * model right before drawing (gp.ab). Fields are decoded by the original {@code lw.ag} if3
 * decoder from archive 3, never typed in by hand.
 */
final class WidgetExport
{
    final RenderExport export;

    WidgetExport(RenderExport export) { this.export = export; }

    /** Interface groups whose model components are exported. */
    static final int[] GROUPS = {679};

    void run(String[] args) throws Exception
    {
        vp archive = export.cache.archive(3);
        Method file = va.class.getDeclaredMethod("tc", va.class, int.class, int.class, int.class);
        file.setAccessible(true);
        Method decodeIf3 = lw.class.getDeclaredMethod("ag", xy.class, int.class);
        decodeIf3.setAccessible(true);
        Method fileCount = va.class.getDeclaredMethod("bh", int.class, int.class);
        fileCount.setAccessible(true);
        List<Object> widgets = new ArrayList<>();
        for (int group : GROUPS)
        {
            int count = (Integer) fileCount.invoke(archive, group, -1891658016);
            for (int child = 0; child < count; child++)
            {
                byte[] data = (byte[]) file.invoke(null, archive, group, child, -600631346);
                if (data == null || data[0] != -1) continue;
                lw widget = new lw();
                Field id = lw.class.getDeclaredField("bk");
                id.setAccessible(true);
                id.setInt(widget, ((group << 16) | child) * 519254441);
                decodeIf3.invoke(widget, new xy(data), 1218236279);
                if (widget.getType() != 6) continue;
                Map<String, Object> record = new LinkedHashMap<>();
                record.put("id", (group << 16) | child);
                record.put("group", group);
                record.put("child", child);
                record.put("parent", widget.getParentId());
                record.put("type", widget.getType());
                record.put("content_type", widget.getContentType());
                record.put("original_x", widget.getOriginalX());
                record.put("original_y", widget.getOriginalY());
                record.put("width", widget.getOriginalWidth());
                record.put("height", widget.getOriginalHeight());
                record.put("x_mode", widget.getXPositionMode());
                record.put("y_mode", widget.getYPositionMode());
                record.put("model_type", decode(widget, "di", -910601));
                record.put("model_id", widget.getModelId());
                record.put("offset_x2", decode(widget, "dw", -1469553671));
                record.put("offset_y2", decode(widget, "dh", -30521739));
                record.put("rotation_x", decode(widget, "de", -1246332317));
                record.put("rotation_y", decode(widget, "dn", 827587461));
                record.put("rotation_z", decode(widget, "dz", -1008620329));
                record.put("model_zoom", decode(widget, "dv", 245118287));
                record.put("animation", decode(widget, "dk", 2114568197));
                record.put("ortho", bool(widget, "gu"));
                record.put("rasterizer_zoom", 512);
                widgets.add(record);
            }
        }
        export.manifest.put("model_widgets", widgets);
        System.out.println("WIDGETS " + widgets.size());
    }

    static int decode(lw widget, String name, int multiplier) throws Exception
    {
        Field field = lw.class.getDeclaredField(name);
        field.setAccessible(true);
        int raw = field.getInt(widget);
        return multiplier == 0 ? raw : raw * multiplier;
    }

    static boolean bool(lw widget, String name) throws Exception
    {
        Field field = lw.class.getDeclaredField(name);
        field.setAccessible(true);
        return field.getBoolean(widget);
    }
}
