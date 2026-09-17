import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.awt.image.BufferedImage;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;
import net.runelite.api.Animation;
import net.runelite.api.Client;
import net.runelite.api.FontTypeFace;
import net.runelite.api.Model;
import net.runelite.api.ModelData;
import net.runelite.api.SpritePixels;
import net.runelite.api.hooks.Callbacks;
import net.runelite.cache.definitions.SpriteDefinition;
import net.runelite.cache.definitions.loaders.SpriteLoader;

/** Calls the unmodified pinned injected runtime. No original game loop or network login runs. */
public final class OriginalCapture
{
    static final Gson JSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
    static final int WIDTH = 1920, HEIGHT = 1080, BACKGROUND = 0x303030;
    final Client game;
    final OriginalCache cache;
    final Path output;
    final List<Object> captures = new ArrayList<>();
    final Map<Integer, zv> fonts = new LinkedHashMap<>();

    public static void main(String[] args)
    {
        try
        {
            long seed = Long.getLong("clubscape.capture.seed", 0L);
            Field randomField = Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
            randomField.setAccessible(true);
            ((java.util.Random) randomField.get(null)).setSeed(seed);
            try (OriginalCache cache = new OriginalCache(Path.of(args[0])))
            {
                ImageIO.setUseCache(false);
                OriginalCapture capture = new OriginalCapture(cache, Path.of(args[2]));
                if (args.length > 3 && args[3].equals("scenes")) new WorldCapture(capture).run();
                else if (args.length > 3 && args[3].equals("title")) new TitleCapture(capture).run();
                else if (args.length > 3 && args[3].equals("hud")) new HudCapture(capture).run();
                else if (args.length > 3 && args[3].equals("all"))
                {
                    capture.run();
                    new WorldCapture(capture).run();
                    new TitleCapture(capture).run();
                }
                else capture.run();
                Files.writeString(capture.output.resolve("captures.json"), JSON.toJson(map("schema_version", 1,
                    "runtime", "injected-client-1.12.38", "configured_game_revision", capture.game.getRevision(),
                    "source_cache_id", 2695, "fixture_random_seed", seed, "profile", args.length > 3 ? args[3] : "models",
                    "capture_classification", "Controlled original-runtime fixtures; no game loop, authentication or terms acceptance.",
                    "native_indexes", cache.provenance, "gpu_plugin_enabled", capture.game.isGpu(),
                    "audio", "Graphics-only: native animation stepping with a silent event consumer; no synthesis or playback.",
                    "captures", capture.captures, "owner_reference_pack_approved", false)));
            }
            System.exit(0); // Also closes the original client's otherwise-idle secure-random executor.
        }
        catch (Throwable error)
        {
            error.printStackTrace();
            System.exit(1);
        }
    }

    OriginalCapture(OriginalCache cache, Path output) throws Exception
    {
        this.cache = cache;
        this.output = output;
        client implementation = new client();
        game = implementation;
        oe.cz = implementation;
        int inverse = BigInteger.valueOf(2098754687).modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
        aal.ae = 240 * inverse;
        if (game.getRevision() != 240) throw new IllegalStateException("Original revision binding failed");
        implementation.km = Thread.currentThread();
        implementation.xr = (Callbacks) Proxy.newProxyInstance(Callbacks.class.getClassLoader(),
            new Class<?>[]{Callbacks.class}, (proxy, method, values) ->
            {
                if (method.getReturnType() == boolean.class) return false;
                if (method.getReturnType() == int.class) return 0;
                if (method.getReturnType() == long.class) return 0L;
                return null;
            });
        vp[] archiveSlots = new vp[25];
        for (int index : new int[]{0, 1, 2, 5, 7, 8, 9, 10, 13, 22})
        {
            archiveSlots[index] = cache.archive(index);
        }
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("ov") && field.getType() == vp[].class)
            {
                field.setAccessible(true);
                vp[] originalSlots = (vp[]) field.get(null);
                if (originalSlots == null || originalSlots.length <= 22)
                {
                    throw new IllegalStateException("Native archive slot layout differs: " + (originalSlots == null ? -1 : originalSlots.length));
                }
                System.arraycopy(archiveSlots, 0, originalSlots, 0, Math.min(originalSlots.length, archiveSlots.length));
            }
        }
        bind("pl", "ae", 2);
        bind("pl", "ab", 7);
        bind("ot", "du", 2);
        bind("gu", "dt", 7);
        bind("ou", "av", 2);
        bind("kp", "at", 0);
        bind("iy", "am", 1);
        bind("gn", "an", 22);
        bind("kd", "cl", 7);
        bind("ak", "cq", 2);
        bind("pt", "ae", 2);
        if (!game.getObjectDefinition(1277).getName().equals("Tree"))
        {
            throw new IllegalStateException("Native source object definition binding failed");
        }
        ec textureProvider = new ec(cache.archive(9), cache.archive(8), 64, 0.8, 128);
        fh.af(textureProvider);
        WorldCapture.staticField(rs.class, "mr", ec.class, textureProvider);
        fh.ae(0.8);
        for (int id : new int[]{494, 495, 496}) fonts.put(id, font(id));
        ne.dh = fonts.get(494);
        ab.kn = new cy();
        WorldCapture.staticField(at.class, "kt", aax.class, new MutedAnimationAudio());
    }

    private void bind(String owner, String name, int index) throws Exception
    {
        Field field = Class.forName(owner).getDeclaredField(name);
        field.setAccessible(true);
        field.set(null, cache.archive(index));
    }

    private zv font(int id) throws Exception
    {
        SpriteDefinition[] sprites = new SpriteLoader().load(id, cache.archive(8).loadData(id, 0));
        if (sprites.length != 256) throw new IllegalStateException("Font glyph count");
        int[] xs = new int[256], ys = new int[256], widths = new int[256], heights = new int[256];
        byte[][] masks = new byte[256][];
        for (int i = 0; i < 256; i++)
        {
            SpriteDefinition sprite = sprites[i];
            xs[i] = sprite.getOffsetX(); ys[i] = sprite.getOffsetY();
            widths[i] = sprite.getWidth(); heights[i] = sprite.getHeight();
            masks[i] = sprite.pixelIdx;
        }
        return new zv(cache.archive(13).loadData(id, 0), xs, ys, widths, heights, sprites[0].palette, masks);
    }

    static Map<String, Object> map(Object... entries)
    {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < entries.length; i += 2) result.put((String) entries[i], entries[i + 1]);
        return result;
    }

    static String hash(byte[] data) throws Exception
    {
        return java.util.HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
    }

    int[] target(int width, int height, int background, int zoom)
    {
        tg provider = new tg(width, height, new java.awt.Canvas(), false);
        wo.qi = provider;
        int[] pixels = ((net.runelite.api.BufferProvider) provider).getPixels();
        Arrays.fill(pixels, background);
        fh.ap(pixels, width, height, null);
        game.getRasterizer().resetRasterClipping();
        fh.ao.ok(zoom);
        return pixels;
    }

    private Map<String, Object> geometry(Model model) throws Exception
    {
        int vertices = model.getVerticesCount(), faces = model.getFaceCount();
        if (vertices <= 0 || faces <= 0) throw new IllegalStateException("Empty native model");
        float[][] coordinates = {model.getVerticesX(), model.getVerticesY(), model.getVerticesZ()};
        ByteBuffer encoded = ByteBuffer.allocate(vertices * 12);
        float[] minimum = {Float.MAX_VALUE, Float.MAX_VALUE, Float.MAX_VALUE};
        float[] maximum = {-Float.MAX_VALUE, -Float.MAX_VALUE, -Float.MAX_VALUE};
        for (int i = 0; i < vertices; i++)
        {
            for (int axis = 0; axis < 3; axis++)
            {
                float value = coordinates[axis][i];
                if (!Float.isFinite(value) || Math.abs(value) > 100000) throw new IllegalStateException("Native model bounds");
                minimum[axis] = Math.min(minimum[axis], value);
                maximum[axis] = Math.max(maximum[axis], value);
                encoded.putFloat(value);
            }
        }
        for (int[] indices : new int[][]{model.getFaceIndices1(), model.getFaceIndices2(), model.getFaceIndices3()})
        {
            if (indices.length < faces) throw new IllegalStateException("Native face cardinality");
            for (int i = 0; i < faces; i++) if (indices[i] < 0 || indices[i] >= vertices) throw new IllegalStateException("Native triangle index");
        }
        return map("vertices", vertices, "faces", faces, "bounds_min", minimum, "bounds_max", maximum,
            "vertex_xyz_float32_be_sha256", hash(encoded.array()));
    }

    void save(String name, int[] pixels, int width, int height, int background,
                      String kind, Object source, Object settings, int minimumPixels) throws Exception
    {
        int changed = 0;
        int[] bbox = {width, height, -1, -1};
        HashSet<Integer> colors = new HashSet<>();
        if (pixels.length < width * height) throw new IllegalStateException("Native framebuffer size mismatch");
        int[] rgba = Arrays.copyOf(pixels, width * height);
        for (int i = 0; i < rgba.length; i++)
        {
            int rgb = pixels[i] & 0xffffff;
            boolean visible = background >= 0 ? rgb != background : pixels[i] != 0;
            if (visible)
            {
                changed++;
                bbox[0] = Math.min(bbox[0], i % width); bbox[1] = Math.min(bbox[1], i / width);
                bbox[2] = Math.max(bbox[2], i % width); bbox[3] = Math.max(bbox[3], i / width);
                colors.add(rgb);
            }
            rgba[i] = background >= 0 || pixels[i] != 0 ? 0xff000000 | rgb : 0;
        }
        if (changed < minimumPixels || colors.size() < 2) throw new IllegalStateException("Insufficient original-render pixels: " + name);
        BufferedImage image = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
        image.setRGB(0, 0, width, height, rgba, 0, width);
        Path file = output.resolve(name + ".png");
        Files.createDirectories(file.getParent());
        if (!ImageIO.write(image, "png", file.toFile())) throw new IllegalStateException("PNG encoder");
        BufferedImage decoded = ImageIO.read(file.toFile());
        if (decoded == null || decoded.getWidth() != width || decoded.getHeight() != height
            || !Arrays.equals(rgba, decoded.getRGB(0, 0, width, height, null, 0, width)))
        {
            throw new IllegalStateException("Native pixel capture changed during PNG encoding");
        }
        ByteBuffer pixelBytes = ByteBuffer.allocate(rgba.length * 4);
        for (int pixel : rgba) pixelBytes.putInt(pixel);
        captures.add(map("path", name + ".png", "kind", kind, "width", width, "height", height,
            "size_bytes", Files.size(file), "sha256", hash(Files.readAllBytes(file)),
            "pixel_argb32_be_sha256", hash(pixelBytes.array()), "png_roundtrip_exact", true,
            "nonbackground_pixels", changed, "nonbackground_colors", colors.size(), "pixel_bounds", bbox,
            "source", source, "settings", settings, "authenticated_source_journey", false));
        System.out.println("CAPTURE " + name + " " + width + "x" + height + " pixels=" + changed);
    }

    private void model(String name, Model model, Object source, int yaw, int zCamera, int yCamera) throws Exception
    {
        int zoom = 1024;
        int[] pixels = target(WIDTH, HEIGHT, BACKGROUND, zoom);
        model.drawFrustum(0, yaw, 0, 128, 0, yCamera, zCamera);
        Map<String, Object> settings = map("render_method", "net.runelite.api.Model.drawFrustum",
            "draw_arguments", new int[]{0, yaw, 0, 128, 0, yCamera, zCamera}, "zoom", zoom,
            "clip_mid", new int[]{WIDTH / 2, HEIGHT / 2}, "brightness", 0.8, "texture_resolution", 128,
            "background_rgb", BACKGROUND, "geometry", geometry(model),
            "classification", "Controlled original software-renderer model fixture, not an in-world camera observation.");
        save(name, pixels, WIDTH, HEIGHT, BACKGROUND, "original-runtime-model", source, settings, 1000);
    }

    private void run() throws Exception
    {
        for (int yaw : new int[]{0, 256, 512, 1024})
        {
            ModelData tree = game.loadModelData(1570);
            if (tree.getVerticesCount() != 90 || tree.getFaceCount() != 110) throw new IllegalStateException("Original tree topology mismatch");
            tree.recolor((short) 3470, (short) 5029);
            model("models/tree-1277-yaw-" + yaw, tree.light(64, 768, -50, -10, -50),
                map("object_id", 1277, "model_id", 1570, "recolor", new int[]{3470, 5029},
                    "lighting", new int[]{64, 768, -50, -10, -50}), yaw, 750, 250);
        }
        for (int npcId : new int[]{3028, 2063})
        {
            pl definition = (pl) game.getNpcDefinition(npcId);
            for (int sequenceId : npcId == 3028 ? new int[]{6181, 6180} : new int[]{5668, 5666})
            {
                ou sequence = (ou) game.loadAnimation(sequenceId);
                if (sequence == null) throw new IllegalStateException("Missing native animation");
                Animation animation = sequence;
                int frameCount = animation.getNumFrames();
                if (frameCount <= 0 || frameCount > 200) throw new IllegalStateException("Unexpected native frame count");
                for (int frame = 0; frame < frameCount; frame++)
                {
                    Model model = definition.ag(sequence, frame, null, -1, null, -520150610);
                    model("models/npc-" + npcId + "-sequence-" + sequenceId + "-frame-" + frame, model,
                        map("npc_id", npcId, "sequence_id", sequenceId, "frame_index", frame,
                            "frame_lengths_client_cycles", animation.getFrameLengths(),
                            "native_assembly", "pl.ag: original model assembly, recolors, lighting, animation and NPC scale"),
                        256, npcId == 3028 ? 650 : 400, npcId == 3028 ? 240 : 160);
                }
            }
        }
        for (int id : new int[]{1265, 436, 438, 2349, 1351, 590, 303, 317, 315, 1205, 1277, 841, 882,
            556, 558, 995, 1511, 1925, 1927, 1931, 1933, 1944, 1947, 2309})
        {
            int quantity = id == 995 ? 1000 : 1;
            SpritePixels icon = game.createItemSprite(id, quantity, 1, SpritePixels.DEFAULT_SHADOW_COLOR, id == 995 ? 1 : 0, false, 512);
            if (icon == null) throw new IllegalStateException("Native item icon missing " + id);
            save("items/" + id, icon.getPixels(), icon.getWidth(), icon.getHeight(), -1, "original-runtime-item-icon",
                map("item_id", id, "quantity", quantity),
                map("render_method", "Client.createItemSprite", "outline", 1, "shadow", SpritePixels.DEFAULT_SHADOW_COLOR,
                    "quantity_mode", id == 995 ? 1 : 0, "noted", false, "zoom", 512), 20);
        }
        int[] ui = target(WIDTH, HEIGHT, 0, 512);
        List<Object> fontMetrics = new ArrayList<>();
        for (int id : fonts.keySet())
        {
            FontTypeFace font = fonts.get(id);
            int y = 180 + (id - 494) * 180;
            String text = "Old School RuneScape  |  Attack: 1  Hitpoints: 10  Coins: 1,000";
            font.drawWidgetText(text, 80, y, 1100, 80, 0xffff00, 0, 256, 0, 0, 0);
            font.drawWidgetText("ABCDEFGHIJKLMNOPQRSTUVWXYZ abcdefghijklmnopqrstuvwxyz 0123456789", 80, y + 60, 1100, 80,
                0xffffff, 0, 256, 0, 0, 0);
            fontMetrics.add(map("font_id", id, "text", text, "text_width", font.getTextWidth(text), "baseline", font.getBaseline()));
        }
        game.getSprites(cache.archive(8), 499, 0)[0].drawAt(1450, 170);
        game.getSprites(cache.archive(8), 500, 0)[0].drawAt(1520, 420);
        save("components/native-fonts-and-title-sprites", ui, WIDTH, HEIGHT, 0, "original-runtime-component-fixture",
            map("font_ids", fonts.keySet(), "sprite_ids", new int[]{499, 500}),
            map("font_metrics", fontMetrics, "native_render_methods", new String[]{"FontTypeFace.drawWidgetText", "SpritePixels.drawAt"},
                "layout", "Explicit test sheet, not a stock login or HUD composition."), 2000);
    }
}
