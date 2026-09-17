import java.nio.file.Files;
import java.util.LinkedHashMap;
import net.runelite.cache.definitions.loaders.AreaLoader;

/** Original map-element identities and their primary source sprite, not UI panel crops. */
public final class UiMapIcons
{
    static void run(OriginalCapture capture) throws Exception
    {
        var loader = new AreaLoader();
        var elements = new LinkedHashMap<Integer, Object>();
        for (int id : capture.cache.archive(2).getFileIds(35))
        {
            byte[] raw = capture.cache.archive(2).loadData(35, id);
            var definition = loader.load(raw, id);
            int sprite = definition.getSpriteId();
            UiAssetExport.sprite(capture, sprite);
            elements.put(id, OriginalCapture.map("sourceId", id, "sprite", sprite, "name", definition.getName(),
                "sourceSha256", OriginalCapture.hash(raw)));
        }
        Files.writeString(capture.output.resolve("map-elements.json"), OriginalCapture.JSON.toJson(elements));
    }
}
