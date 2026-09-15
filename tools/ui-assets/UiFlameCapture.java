import java.lang.reflect.Field;
import java.nio.file.Files;
import java.util.ArrayList;
import net.runelite.cache.definitions.loaders.SpriteLoader;

/** Original title-effect-only fixtures. No account/title handlers, panel crops or audio. */
public final class UiFlameCapture
{
    static void run(OriginalCapture capture) throws Exception
    {
        var directory = UiAssetExport.directory(capture, "flames");
        TitleCapture title = new TitleCapture(capture);
        var indexed = TitleCapture.class.getDeclaredMethod("indexedFrames", String.class);
        indexed.setAccessible(true);
        yz[] runes = (yz[]) indexed.invoke(title, "runes");
        aam defaults = new aam();
        defaults.az(capture.cache.archive(17), -243617527);
        Field randomField = Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
        randomField.setAccessible(true);
        ((java.util.Random) randomField.get(null)).setSeed(0);
        cs effect = new cs(runes, defaults.ac);
        WorldCapture.staticField(cs.class, "zy", boolean.class, true);
        var masks = new ArrayList<>();
        int runeId = capture.cache.store.findIndex(8).findArchiveByName("runes").getArchiveId();
        var definitions = new SpriteLoader().load(runeId, capture.cache.archive(8).loadData(runeId, 0));
        for (var rune : definitions)
        {
            int[] mask = new int[rune.pixelIdx.length];
            for (int i = 0; i < mask.length; i++) mask[i] = rune.pixelIdx[i] == 0 ? 0 : 1;
            masks.add(OriginalCapture.map("width", rune.getWidth(), "height", rune.getHeight(),
                "offsetX", rune.getOffsetX(), "offsetY", rune.getOffsetY(), "mask", mask));
        }
        Files.writeString(capture.output.resolve("flames.json"), OriginalCapture.JSON.toJson(OriginalCapture.map(
            "width", 128, "height", 256, "cycleMs", 20, "leftOffset", -22, "rightOffset", 659,
            "palettes", new int[][]{effect.ar, effect.ac, effect.au}, "runes", masks,
            "source", "Original cs constructor/ae/al/je/as/bu through cs.fl; source indexed runes and defaults palettes")));
        var records = new ArrayList<>();
        int[] frames = {1, 2, 3, 10, 40, 128, 257, 320, 640, 1024, 2048};
        for (int cycle = 0; cycle <= 2048; cycle++)
        {
            WorldCapture.logicalInt(null, client.class, "cm", 1612595797, cycle);
            int[] pixels = capture.target(765, 280, 0x203040, 512);
            cs.fl(effect, -22, cycle, 1698595949);
            cs.fl(effect, 659, cycle, 1698595949);
            if (java.util.Arrays.binarySearch(frames, cycle) >= 0)
            {
                String name = "cycle-" + cycle;
                UiAssetExport.png(directory.resolve(name + ".png"), pixels, 765, 280, true);
                records.add(OriginalCapture.map("name", name, "cycle", cycle, "seed", 0, "background", 0x203040,
                    "greenFade", effect.aq * -1551918995, "blueFade", effect.ad * 1386667955,
                    "scope", "Original procedural title component, not a finished panel or gameplay source state."));
            }
        }
        Files.writeString(directory.resolve("captures.json"), OriginalCapture.JSON.toJson(records));
        WorldCapture.staticField(vp.class, "di", zv.class, capture.fonts.get(494));
        WorldCapture.staticField(client.class, "ds", boolean.class, true);
        int[] banner = capture.target(300, 70, 0x203040, 512);
        lu.bz("Connection lost<br>Please wait - attempting to reestablish", false, 752162477);
        UiAssetExport.png(directory.resolve("reconnect.png"), banner, 300, 70, true);
    }
}
