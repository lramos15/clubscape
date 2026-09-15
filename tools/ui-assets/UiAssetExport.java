import java.awt.image.BufferedImage;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import javax.imageio.ImageIO;
import net.runelite.api.SpritePixels;
import net.runelite.api.widgets.Widget;
import net.runelite.cache.definitions.SpriteDefinition;
import net.runelite.cache.definitions.loaders.SpriteLoader;
import net.runelite.cache.definitions.loaders.DBRowLoader;

/** Instrumentation of the pinned offline source host, never code loaded by the browser. */
public final class UiAssetExport
{
    static final TreeSet<Integer> sprites = new TreeSet<>();
    static final TreeSet<Integer> fonts = new TreeSet<>();
    static final TreeSet<String> models = new TreeSet<>();
    static final List<Object> rendererPreviews = new ArrayList<>();
    static final Map<String, Object> portraitMetadata = new LinkedHashMap<>();

    static Path directory(OriginalCapture capture, String name) throws Exception
    {
        Path path = capture.output.resolve(name);
        Files.createDirectories(path);
        return path;
    }

    static void png(Path path, int[] pixels, int width, int height, boolean opaque) throws Exception
    {
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
        int[] argb = pixels.clone();
        for (int i = 0; i < argb.length; i++)
        {
            if (opaque || argb[i] != 0) argb[i] |= 0xff000000;
        }
        image.setRGB(0, 0, width, height, argb, 0, width);
        ImageIO.write(image, "png", path.toFile());
    }

    static Map<String, Object> widget(Widget w)
    {
        return OriginalCapture.map(
            "id", w.getId(), "index", w.getIndex(), "parent", w.getParentId(),
            "x", w.getCanvasLocation().getX(), "y", w.getCanvasLocation().getY(),
            "width", w.getWidth(), "height", w.getHeight(), "type", w.getType(),
            "contentType", w.getContentType(), "text", w.getText(), "sprite", w.getSpriteId(),
            "item", w.getItemId(), "item_quantity", w.getItemQuantity(),
            "font", w.getFontId(), "color", w.getTextColor(), "shadow", w.getTextShadowed(),
            "lineHeight", w.getLineHeight(), "xText", w.getXTextAlignment(), "yText", w.getYTextAlignment(),
            "opacity", w.getOpacity(), "filled", w.isFilled(), "tiling", w.getSpriteTiling(),
            "flipX", w.isFlippedHorizontally(), "flipY", w.isFlippedVertically(), "border", w.getBorderType(),
            "scrollX", w.getScrollX(), "scrollY", w.getScrollY(),
            "scrollWidth", w.getScrollWidth(), "scrollHeight", w.getScrollHeight(),
            "originalX", w.getOriginalX(), "originalY", w.getOriginalY(),
            "originalWidth", w.getOriginalWidth(), "originalHeight", w.getOriginalHeight(),
            "xMode", w.getXPositionMode(), "yMode", w.getYPositionMode(),
            "widthMode", w.getWidthMode(), "heightMode", w.getHeightMode(),
            "actions", w.getActions(), "name", w.getName(), "targetVerb", w.getTargetVerb(),
            "quantityMode", w.getItemQuantityMode(), "dragTime", w.getDragDeadTime(), "dragZone", w.getDragDeadZone(),
            "modelType", w.getModelType(), "model", w.getModelId(),
            "modelZoom", w.getModelZoom(), "modelRotation", new int[]{w.getRotationX(), w.getRotationY(), w.getRotationZ()},
            "onOp", w.getOnOpListener(), "noClickThrough", w.getNoClickThrough(), "if3", w.isIf3());
    }

    static void sprite(OriginalCapture capture, int id) throws Exception
    {
        if (id < 0 || !sprites.add(id)) return;
        byte[] raw = capture.cache.archive(8).loadData(id, 0);
        if (raw == null) throw new IllegalStateException("Missing original sprite " + id);
        SpriteDefinition[] source = new SpriteLoader().load(id, raw);
        List<Object> frames = new ArrayList<>();
        int width = 0, height = 0, rowHeight = 0, x = 0, y = 0;
        for (SpriteDefinition s : source)
        {
            if (x + s.getWidth() > 1024 && x > 0) { x = 0; y += rowHeight; rowHeight = 0; }
            frames.add(OriginalCapture.map("x", x, "y", y, "width", s.getWidth(), "height", s.getHeight(),
                "offsetX", s.getOffsetX(), "offsetY", s.getOffsetY(),
                "canvasWidth", s.getMaxWidth(), "canvasHeight", s.getMaxHeight()));
            width = Math.max(width, x + s.getWidth());
            rowHeight = Math.max(rowHeight, s.getHeight());
            height = Math.max(height, y + rowHeight);
            x += s.getWidth();
        }
        BufferedImage atlas = new BufferedImage(Math.max(1, width), Math.max(1, height), BufferedImage.TYPE_INT_ARGB);
        for (int i = 0; i < source.length; i++)
        {
            SpriteDefinition s = source[i];
            @SuppressWarnings("unchecked") Map<String, Object> f = (Map<String, Object>) frames.get(i);
            if (s.getWidth() == 0 || s.getHeight() == 0) continue;
            int[] pixels = s.getPixels().clone();
            for (int p = 0; p < pixels.length; p++)
            {
                if (s.pixelIdx != null && s.pixelIdx[p] != 0) pixels[p] |= 0xff000000;
            }
            atlas.setRGB((int) f.get("x"), (int) f.get("y"), s.getWidth(), s.getHeight(), pixels, 0, s.getWidth());
        }
        Path out = directory(capture, "sprites");
        ImageIO.write(atlas, "png", out.resolve(id + ".png").toFile());
        Files.writeString(out.resolve(id + ".json"), OriginalCapture.JSON.toJson(
            OriginalCapture.map("sourceId", id, "sourceRawSha256", OriginalCapture.hash(raw), "frames", frames)));
    }

    static void font(OriginalCapture capture, int id) throws Exception
    {
        if (id < 0 || !fonts.add(id)) return;
        byte[] raw = capture.cache.archive(13).loadData(id, 0);
        if (raw == null || raw.length != 257)
            throw new IllegalStateException("Unimplemented original kerning-format font " + id);
        List<Integer> advances = new ArrayList<>();
        for (int i = 0; i < 256; i++) advances.add(raw[i] & 255);
        Path out = directory(capture, "fonts");
        Files.writeString(out.resolve(id + ".json"), OriginalCapture.JSON.toJson(
            OriginalCapture.map("sourceId", id, "sourceRawSha256", OriginalCapture.hash(raw),
                "ascent", raw[256] & 255, "advances", advances)));
        sprite(capture, id);
    }

    static void visit(Widget w, IdentityHashMap<Widget, Boolean> visited, List<Widget> result)
    {
        if (w == null || visited.put(w, true) != null) return;
        result.add(w);
        Widget[] children = w.getChildren();
        if (children != null) for (Widget child : children) visit(child, visited, result);
    }

    static void rendererPreviewBoundary(OriginalCapture capture, HudCapture hud, int group) throws Exception
    {
        List<Widget> all = new ArrayList<>();
        IdentityHashMap<Widget, Boolean> visited = new IdentityHashMap<>();
        for (lw widget : hud.widgets.ax[group]) visit(widget, visited, all);
        for (Widget widget : all)
        {
            if (widget.getType() != 6 || widget.getModelType() == 2
                || (widget.getModelType() == 1 && widget.getModelId() == -1 && widget.getContentType() == 0)) continue;
            rendererPreviews.add(OriginalCapture.map("group", group, "widget", widget(widget),
                "status", "RENDERER_BOUNDARY_UNIMPLEMENTED",
                "reason", "Original human player preview is not the approved penguin player. Only source UI controls/frames are exported here."));
            widget.setContentType(0).setModelType(1).setModelId(-1);
        }
        Files.writeString(capture.output.resolve("renderer-previews.json"), OriginalCapture.JSON.toJson(rendererPreviews));
    }

    static void frame(OriginalCapture capture, HudCapture hud, String name, List<Object> state) throws Exception
    {
        Path out = directory(capture, "styles");
        Files.writeString(out.resolve(name + ".json"), OriginalCapture.JSON.toJson(state));
        for (Object value : state)
        {
            @SuppressWarnings("unchecked") Map<String, Object> w = (Map<String, Object>) value;
            sprite(capture, (int) w.get("sprite"));
            font(capture, (int) w.get("font"));
        }
        List<Widget> all = new ArrayList<>();
        IdentityHashMap<Widget, Boolean> visited = new IdentityHashMap<>();
        for (lw[] group : hud.widgets.ax)
            if (group != null) for (lw w : group) visit(w, visited, all);
        List<Widget> worldWidgets = all.stream().filter(w -> !w.isHidden() && w.getContentType() == 1337).toList();
        for (Widget w : worldWidgets) w.setHidden(true);
        try
        {
            int[] ui = capture.target(1920, 1080, 0, 512);
            qi.ck.az(1920, 1080, hud.widgets, 1, 2, -293044276);
            png(directory(capture, "ui-only").resolve(name + ".png"), ui, 1920, 1080, false);
        }
        finally { for (Widget w : worldWidgets) w.setHidden(false); }
        List<Widget> heads = all.stream().filter(w -> !w.isHidden() && w.getType() == 6
            && w.getModelType() == 2 && w.getWidth() > 0 && w.getHeight() > 0).toList();
        for (Widget head : heads)
        {
            String key = "npc-" + head.getModelId();
            if (!models.add(key)) continue;
            IdentityHashMap<Widget, Boolean> hidden = new IdentityHashMap<>();
            for (Widget w : all)
            {
                hidden.put(w, w.isSelfHidden());
                if (w != head && (w.getType() == 3 || w.getType() == 4 || w.getType() == 5
                    || w.getType() == 6 || w.getContentType() != 0)) w.setHidden(true);
            }
            try
            {
                int[] pixels = capture.target(1920, 1080, 0x123456, 512);
                qi.ck.az(1920, 1080, hud.widgets, 1, 2, -293044276);
                Widget clip = head.getParent();
                int width = clip.getWidth(), height = clip.getHeight();
                int[] component = new int[width * height];
                int x = clip.getCanvasLocation().getX(), y = clip.getCanvasLocation().getY();
                for (int row = 0; row < height; row++)
                    System.arraycopy(pixels, (y + row) * 1920 + x, component, row * width, width);
                for (int pixel = 0; pixel < component.length; pixel++)
                    component[pixel] = component[pixel] == 0x123456 ? 0 : component[pixel] | 0xff000000;
                png(directory(capture, "portraits").resolve(key + ".png"), component, width, height, false);
                portraitMetadata.put(key, OriginalCapture.map("asset", "ui/portraits/" + key + ".png",
                    "offsetX", x - head.getCanvasLocation().getX(), "offsetY", y - head.getCanvasLocation().getY()));
            }
            finally { hidden.forEach((w, value) -> w.setHidden(value)); }
        }
        Files.writeString(capture.output.resolve("portraits.json"), OriginalCapture.JSON.toJson(portraitMetadata));
    }

    static void extras(OriginalCapture capture) throws Exception
    {
        for (int id : new int[]{494, 495, 496, 497}) font(capture, id);
        for (int id : new int[]{498, 499, 500, 501, 697, 699, 1211, 1213, 811, 1649, 2133})
            sprite(capture, id);
        int switchWorld = capture.cache.store.findIndex(8).findArchiveByName("sl_button").getArchiveId();
        sprite(capture, switchWorld);
        Files.writeString(capture.output.resolve("named-sprites.json"), OriginalCapture.JSON.toJson(
            OriginalCapture.map("worldSwitch", switchWorld)));
        int titleId = capture.cache.store.findIndex(10).findArchiveByName("title.jpg").getArchiveId();
        ym title = it.az(capture.cache.archive(10).loadData(titleId, 0), 1951476339);
        png(capture.output.resolve("title-background.png"), title.getPixels(), title.getWidth(), title.getHeight(), true);
        int[][] items = OriginalCapture.JSON.fromJson(
            Files.readString(Path.of(System.getProperty("clubscape.ui.items"))), int[][].class);
        Path out = directory(capture, "items");
        Map<String, Object> metadata = new LinkedHashMap<>();
        for (int[] request : items)
        {
            var definition = capture.game.getItemDefinition(request[0]);
            metadata.put(Integer.toString(request[0]), OriginalCapture.map("name", definition.getName(),
                "stackable", definition.isStackable() ? 1 : 0, "interfaceOptions", definition.getInventoryActions()));
            for (int outline : new int[]{1, 2})
            {
                SpritePixels icon = capture.game.createItemSprite(request[0], request[1], outline,
                    0x333333, 0, false, 512);
                if (icon == null) throw new IllegalStateException("Missing native item icon " + request[0]);
                png(out.resolve(request[0] + "-" + request[1] + "-" + outline + ".png"),
                    icon.getPixels(), icon.getWidth(), icon.getHeight(), false);
            }
        }
        Files.writeString(capture.output.resolve("item-metadata.json"), OriginalCapture.JSON.toJson(metadata));
        Path maps = directory(capture, "minimaps");
        aam graphics = new aam();
        graphics.az(capture.cache.archive(17), -243617527);
        var dotArchive = capture.cache.store.findIndex(8).findArchiveByName("mapdots");
        if (dotArchive == null) throw new IllegalStateException("Original named mapdots sprite archive is missing");
        int dotGroup = dotArchive.getArchiveId();
        SpriteDefinition[] dots = new SpriteLoader().load(dotGroup, capture.cache.archive(8).loadData(dotGroup, 0));
        for (int i = 0; i < dots.length; i++)
        {
            SpriteDefinition dot = dots[i];
            png(maps.resolve("dot-" + i + ".png"), dot.getPixels(), dot.getWidth(), dot.getHeight(), false);
        }
        List<Object> categories = new ArrayList<>();
        DBRowLoader rows = new DBRowLoader();
        for (int id : capture.cache.archive(2).getFileIds(38))
        {
            var row = rows.load(id, capture.cache.archive(2).loadData(38, id));
            if (row.getTableId() != 78) continue;
            categories.add(row);
            Object[][] values = row.getColumnValues();
            if (values.length > 1 && values[1] != null)
                for (int i = 3; i < values[1].length; i += 4) sprite(capture, ((Number) values[1][i]).intValue());
        }
        Files.writeString(capture.output.resolve("combat-categories.json"), OriginalCapture.JSON.toJson(categories));
        png(maps.resolve("compass.png"), gg.mf.getPixels(), gg.mf.getWidth(), gg.mf.getHeight(), false);
        png(maps.resolve("3168-3168-0.png"), rd.ax.getPixels(), rd.ax.getWidth(), rd.ax.getHeight(), true);
        List<Object> mapEntries = new ArrayList<>();
        mapEntries.add(OriginalCapture.map("baseX", 3168, "baseY", 3168, "plane", 0,
            "asset", "ui/minimaps/3168-3168-0.png"));
        WorldCapture world = new WorldCapture(capture);
        java.lang.reflect.Method view = WorldCapture.class.getDeclaredMethod("view",
            String.class, int.class, int.class, int.class, int.class, int.class, int.class, int.class, int.class, boolean.class);
        view.setAccessible(true);
        for (int[] base : new int[][]{{3048, 3032}, {3048, 9456}, {3120, 3240}, {3200, 3168}, {3168, 3168}})
        {
            view.invoke(world, "ui-minimap", base[0], base[1], base[0] + 50, -1300,
                base[1] + 40, base[0] + 50, base[1] + 50, 0, false);
            for (int plane = 0; plane < 3; plane++)
            {
                if (base[0] == 3168 && base[1] == 3168 && plane == 0) continue;
                ym map = new ym(512, 512);
                client.bm(is.dk, map, 4.0, plane, 0, 0, 48, 48);
                String file = base[0] + "-" + base[1] + "-" + plane + ".png";
                png(maps.resolve(file), map.getPixels(), 512, 512, true);
                mapEntries.add(OriginalCapture.map("baseX", base[0], "baseY", base[1], "plane", plane,
                    "asset", "ui/minimaps/" + file));
            }
        }
        Files.writeString(capture.output.resolve("minimaps.json"), OriginalCapture.JSON.toJson(mapEntries));
    }
}
