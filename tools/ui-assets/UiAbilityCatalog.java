import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;

/** Original information-item parameters used by the native UI, not gameplay outcomes. */
public final class UiAbilityCatalog
{
    static void write(OriginalCapture capture) throws Exception
    {
        Map<String, Object> abilities = new LinkedHashMap<>();
        Map<Integer, Integer> informationIds = new LinkedHashMap<>();
        Map<Integer, Integer> visibleSprites = new LinkedHashMap<>();
        for (String name : new String[]{"native-prayer", "native-magic"})
        {
            var records = OriginalCapture.JSON.fromJson(Files.readString(capture.output.resolve("styles/" + name + ".json")),
                com.google.gson.JsonArray.class);
            for (var record : records)
            {
                var widget = record.getAsJsonObject();
                if (widget.get("index").getAsInt() != -1 || widget.get("name").getAsString().isEmpty()) continue;
                int id = widget.get("id").getAsInt(), group = id >>> 16;
                if (group != 541 && group != 218) continue;
                visibleSprites.put(id, widget.get("sprite").getAsInt());
                if (!widget.get("onOp").isJsonNull())
                {
                    var operation = widget.getAsJsonArray("onOp");
                    int at = group == 541 ? 3 : 1;
                    if (operation.size() > at && operation.get(at).isJsonPrimitive())
                        informationIds.put(id, operation.get(at).getAsInt());
                }
            }
        }
        for (int id : capture.cache.archive(2).getFileIds(10))
        {
            var item = capture.game.getItemDefinition(id);
            if (item.getParams() == null) continue;
            int spellWidget = item.getIntValue(596), prayerWidget = item.getIntValue(1751);
            boolean spell = spellWidget >>> 16 == 218;
            boolean prayer = prayerWidget >>> 16 == 541;
            if (!spell && !prayer) continue;
            int widget = spell ? spellWidget : prayerWidget;
            if (!visibleSprites.containsKey(widget)) continue;
            if (informationIds.containsKey(widget) && informationIds.get(widget) != id) continue;
            var runes = new ArrayList<>();
            if (spell)
                for (int[] pair : new int[][]{{365, 366}, {367, 368}, {369, 370}, {606, 607}, {1187, 1188}})
                {
                    int rune = item.getIntValue(pair[0]), quantity = item.getIntValue(pair[1]);
                    if (rune >= 0 && quantity > 0) runes.add(OriginalCapture.map("sourceId", rune, "quantity", quantity));
                }
            int[] graphics = spell ? new int[]{item.getIntValue(597), item.getIntValue(598),
                item.getIntValue(599), item.getIntValue(600)} : new int[]{-1, -1, -1, -1};
            if (spell && !informationIds.containsKey(widget)
                && java.util.Arrays.stream(graphics).noneMatch(graphic -> graphic == visibleSprites.get(widget))) continue;
            for (int graphic : graphics) if (graphic >= 0) UiAssetExport.sprite(capture, graphic);
            abilities.put(Integer.toString(widget), OriginalCapture.map("widget", widget, "sourceItem", id,
                "kind", spell ? "magic" : "prayer", "level", item.getIntValue(spell ? 604 : 1753),
                "category", spell ? item.getIntValue(605) : -1, "rapid", prayer && item.getIntValue(1758) == 1,
                "higherTier", prayer ? item.getIntValue(1760) : -1, "tier", prayer ? item.getIntValue(1759) : -1,
                "sprites", graphics, "runes", runes));
        }
        Files.writeString(capture.output.resolve("abilities.json"), OriginalCapture.JSON.toJson(abilities));
    }
}
