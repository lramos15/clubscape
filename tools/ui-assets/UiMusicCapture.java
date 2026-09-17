import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;
import net.runelite.cache.definitions.loaders.DBRowLoader;

/** Original music row identities and mode artwork; no source unlocks are granted to a live client. */
public final class UiMusicCapture
{
    static void run(OriginalCapture capture, HudCapture hud) throws Exception
    {
        Map<String, Object[]> rows = new LinkedHashMap<>();
        var loader = new DBRowLoader();
        for (int id : capture.cache.archive(2).getFileIds(38))
        {
            byte[] raw = capture.cache.archive(2).loadData(38, id);
            var row = loader.load(id, raw);
            if (row.getTableId() != 44) continue;
            Object[][] values = row.getColumnValues();
            if (values[0] == null || values[4] == null) continue;
            String display = (String) (values[1] == null ? values[0][0] : values[1][0]);
            rows.put(display, new Object[]{id, values[4][0],
                values[2] == null ? "" : values[2][0], OriginalCapture.hash(raw)});
        }
        var source = OriginalCapture.JSON.fromJson(Files.readString(capture.output.resolve("styles/native-music.json")),
            com.google.gson.JsonArray.class);
        var tracks = new ArrayList<>();
        for (var entry : source)
        {
            var widget = entry.getAsJsonObject();
            if (widget.get("id").getAsInt() != (239 << 16 | 11) || widget.get("type").getAsInt() != 4) continue;
            String name = widget.get("text").getAsString();
            Object[] row = rows.get(name);
            if (row == null) throw new IllegalStateException("Native music row identity missing: " + name);
            tracks.add(OriginalCapture.map("row", row[0], "group", row[1], "name", name,
                "hint", row[2], "widgetIndex", widget.get("index").getAsInt(), "sourceSha256", row[3]));
        }
        Files.writeString(capture.output.resolve("music-ui-rows.json"), OriginalCapture.JSON.toJson(tracks));
        int before = capture.game.getVarps()[18];
        var cases = new ArrayList<>();
        for (int mode = 0; mode <= 2; mode++)
        {
            capture.game.getVarps()[18] = mode;
            UiModeCapture.load(hud, 239);
            UiModeCapture.tab(hud, 13);
            capture.game.runScript(907, 161 << 16, 1130);
            String name = "native-music-mode-" + mode;
            UiModeCapture.frame(hud, name, 239);
            cases.add(OriginalCapture.map("case", name, "mode", mode, "varp", 18,
                "scope", "Original mode/skip-button artwork at explicit source-only settings, not gameplay unlock state."));
        }
        capture.game.getVarps()[18] = 0;
        UiModeCapture.load(hud, 239);
        UiModeCapture.tab(hud, 13);
        UiModeCapture.op(capture, capture.game.getWidget(239, 18), 1);
        UiModeCapture.frame(hud, "native-music-filter-open", 239);
        cases.add(OriginalCapture.map("case", "native-music-filter-open", "mode", 0, "dropdown", true,
            "scope", "Actual original expanded list filter, not a declaration of unlocked music."));
        capture.game.getVarps()[18] = before;
        Files.writeString(capture.output.resolve("music-ui-inputs.json"), OriginalCapture.JSON.toJson(cases));
    }
}
