import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import net.runelite.cache.definitions.loaders.DBRowLoader;
import net.runelite.cache.definitions.loaders.GameValLoader;
import net.runelite.cache.definitions.loaders.ScriptLoader;

/** Immutable source contracts identified by the native HUD investigation; no renderer implementation. */
public final class HudSourceCatalog
{
    static int[] write(OriginalCache cache, Path output) throws Exception
    {
        ScriptLoader scripts = new ScriptLoader().configureForRevision(cache.store.findIndex(12).getRevision());
        List<Object> scriptContracts = new ArrayList<>();
        for (int id : new int[]{55, 71, 216, 274, 901, 907, 914, 1074, 1076, 1350, 1356, 5995, 6007, 7593, 7603})
        {
            byte[] bytes = cache.archive(12).loadData(id, 0);
            var script = scripts.load(id, bytes);
            scriptContracts.add(OriginalCapture.map("id", id, "bytes", bytes.length,
                "sha256", OriginalCapture.hash(bytes), "int_arguments", script.getIntArgCount(),
                "string_arguments", script.getObjArgCount(),
                "runelite_overlay_applied", net.runelite.api.overlay.OverlayIndex.hasOverlay(12, id)));
        }
        DBRowLoader rows = new DBRowLoader();
        int questCount = 0, questPoints = 0;
        Object swordCategory = null;
        vp configurations = cache.archive(2);
        for (int id : configurations.getFileIds(38))
        {
            var row = rows.load(id, configurations.loadData(38, id));
            if (id == 3959) swordCategory = row;
            if (row.getTableId() != 0) continue;
            Object[][] values = row.getColumnValues();
            if (values[4] != null && ((Number) values[4][0]).intValue() == 0)
            {
                if (values.length <= 21 || values[21] == null) questCount++;
                if (values[17] != null) questPoints += ((Number) values[17][0]).intValue();
            }
        }
        Files.writeString(output.resolve("hud-input-contract.json"), OriginalCapture.JSON.toJson(OriginalCapture.map(
            "schema_version", 1, "scripts", scriptContracts,
            "sword_category_source_row", swordCategory,
            "quest_table_fields", new GameValLoader().load(10, 0, cache.archive(24).loadData(10, 0)),
            "quest_counter_fixture", OriginalCapture.map("available", questCount, "maximum_points", questPoints,
                "derivation", "Source table0 type0, excluding parented subquests from count but retaining their quest points; not observed server state"),
            "shop_main", OriginalCapture.map("source_script", 1074, "arguments", List.of(3, -1, 0, 1, "General Store"),
                "argument_roles", List.of("source stock container", "no selected item", "custom quantity", "enable Buy50", "fixture title")),
            "shop_side", OriginalCapture.map("source_script", 6007, "source_widget", 301 << 16,
                "scope", "Native shared inventory initializer on original shop-side widget; sale behavior/prices are not asserted"),
            "discovery_note", "Original script1074 identified by source group300 widget references; only required contracts are retained here.",
            "owner_approval", false)));
        return new int[]{questCount, questPoints};
    }
}
