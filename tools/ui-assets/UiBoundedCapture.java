import java.nio.file.Files;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import net.runelite.cache.definitions.loaders.EnumLoader;
import net.runelite.cache.definitions.loaders.StructLoader;

/** The owner-bounded independent UI lane: at most eighteen additional native comparison states. */
public final class UiBoundedCapture
{
    static final int LIMIT = 18;
    static final List<Object> records = new ArrayList<>();
    static final java.util.Set<String> names = new LinkedHashSet<>();

    static void frame(OriginalCapture capture, HudCapture hud, String name, int group, Object inputs) throws Exception
    {
        if (!names.add(name) || names.size() > LIMIT) throw new IllegalStateException("Independent native state ceiling exceeded");
        UiModeCapture.frame(hud, name, group);
        records.add(OriginalCapture.map("case", name, "group", group, "inputs", inputs,
            "scope", "Explicit original controlled UI only; no live gameplay or acceptance."));
    }

    static void settings(OriginalCapture capture, HudCapture hud) throws Exception
    {
        int[] savedBits = {9656, 9665, 17796, 16073, 16074};
        int[] saved = java.util.Arrays.stream(savedBits).map(capture.game::getVarbitValue).toArray();
        int[] audioVars = {3796, 168, 169, 872};
        int[] audioBefore = java.util.Arrays.stream(audioVars).map(id -> capture.game.getVarps()[id]).toArray();
        int[] cameraVars = {73, 74, 1338, 1339, 1340, 1341};
        int[] cameraBefore = java.util.Arrays.stream(cameraVars).map(capture.game::getVarcIntValue).toArray();
        int[] cameraInputs = {300, 300, 200, 600, 200, 600};
        for (int index = 0; index < cameraVars.length; index++) capture.game.setVarcIntValue(cameraVars[index], cameraInputs[index]);
        for (int id : audioVars) capture.game.getVarps()[id] = 100;
        UiModeCapture.tab(hud, 11);
        var panel = capture.game.openInterface(161 << 16 | 16, 134, 0);
        String[] categories = {"activities", "audio", "chat", "controls", "display", "gameplay", "interfaces", "warnings"};
        for (int category = 0; category < categories.length; category++)
        {
            capture.game.setVarbit(9656, category);
            capture.game.setVarbit(9665, 0);
            capture.game.setVarbit(17796, 0);
            capture.game.setVarbit(16073, 0);
            capture.game.setVarbit(16074, 0);
            UiModeCapture.load(hud, 134);
            capture.game.runScript(907, 161 << 16, 1130);
            frame(capture, hud, "bounded-settings-" + categories[category], 134,
                OriginalCapture.map("category", category, "moreInfo", true, "hideLocked", false, "search", null,
                    "sourceAudioPercentages", new int[]{100,100,100,100}, "sourceCameraVarcs", cameraVars,
                    "explicitCameraValuesNotDefaults", cameraInputs));
        }
        for (String term : new String[]{"zoom", "no such setting"})
        {
            capture.game.setVarbit(9656, 4);
            capture.game.setVarbit(9665, 1);
            capture.game.setVarbit(17796, 1);
            capture.game.setVarbit(16073, 1);
            capture.game.setVarbit(16074, 1);
            capture.game.setVarcStrValue(417, term);
            UiModeCapture.load(hud, 134);
            capture.game.runScript(907, 161 << 16, 1130);
            frame(capture, hud, term.equals("zoom") ? "bounded-settings-search-zoom" : "bounded-settings-search-empty", 134,
                OriginalCapture.map("category", 4, "moreInfo", false, "hideLocked", true, "search", term,
                    "sourceAudioPercentages", new int[]{100,100,100,100}, "sourceCameraVarcs", cameraVars,
                    "explicitCameraValuesNotDefaults", cameraInputs));
        }
        capture.game.closeInterface(panel, true);
        for (int index = 0; index < savedBits.length; index++) capture.game.setVarbit(savedBits[index], saved[index]);
        for (int index = 0; index < audioVars.length; index++) capture.game.getVarps()[audioVars[index]] = audioBefore[index];
        for (int index = 0; index < cameraVars.length; index++) capture.game.setVarcIntValue(cameraVars[index], cameraBefore[index]);
        capture.game.runScript(907, 161 << 16, 1130);
    }

    static void settingsDefinitions(OriginalCapture capture) throws Exception
    {
        var enums = new EnumLoader();
        var structs = new StructLoader();
        Map<Integer, Object> definitions = new LinkedHashMap<>();
        Map<Integer, Object> categories = new LinkedHashMap<>();
        Map<Integer, Object> choices = new LinkedHashMap<>();
        var top = enums.load(422, capture.cache.archive(2).loadData(8, 422));
        for (int index = 0; index < top.getKeys().length; index++)
        {
            int id = top.getIntVals()[index];
            byte[] raw = capture.cache.archive(2).loadData(34, id);
            var category = structs.load(id, raw);
            int enumeration = (int) category.getParams().get(745);
            var rows = enums.load(enumeration, capture.cache.archive(2).loadData(8, enumeration));
            categories.put(top.getKeys()[index], OriginalCapture.map("id", id, "definition", category,
                "sourceSha256", OriginalCapture.hash(raw), "settings", rows.getIntVals()));
            for (int settingId : rows.getIntVals())
            {
                if (settingId < 0 || definitions.containsKey(settingId)) continue;
                byte[] source = capture.cache.archive(2).loadData(34, settingId);
                var definition = structs.load(settingId, source);
                definitions.put(settingId, OriginalCapture.map("sourceSha256", OriginalCapture.hash(source), "definition", definition));
                for (int key : new int[]{1091, 1103})
                {
                    Object value = definition.getParams() == null ? null : definition.getParams().get(key);
                    if (value instanceof Integer && (int) value >= 0 && !choices.containsKey((int) value))
                        choices.put((int) value, enums.load((int) value, capture.cache.archive(2).loadData(8, (int) value)));
                }
            }
        }
        Files.writeString(capture.output.resolve("bounded-settings-definitions.json"),
            OriginalCapture.JSON.toJson(OriginalCapture.map("categories", categories, "settings", definitions, "choices", choices)));
    }

    static void bank(OriginalCapture capture, HudCapture hud) throws Exception
    {
        int[] sourceItems = {995,436,438,2349,1265,1351,1927,1933,1944,1947,315,1511};
        for (int slot = 0; slot < 100; slot++) bh.aq(95, slot, slot < sourceItems.length ? sourceItems[slot] : -1,
            slot < sourceItems.length ? slot == 0 ? 12345 : slot + 1 : 0);
        for (int tab = 1; tab <= 9; tab++) capture.game.setVarbit(4170 + tab, tab <= 2 ? 4 : 0);
        capture.game.setVarbit(4150, 0);
        capture.game.setVarbit(3958, 0);
        capture.game.setVarbit(3959, 0);
        capture.game.setVarbit(3755, 0);
        UiModeCapture.tab(hud, 3);
        var main = capture.game.openInterface(161 << 16 | 16, 12, 0);
        var side = capture.game.openInterface(161 << 16 | 74, 15, 1);
        capture.game.runScript(907, 161 << 16, 1130);
        frame(capture, hud, "bounded-bank-tabs", 12,
            OriginalCapture.map("selectedTab", 0, "tabSizes", new int[]{4,4}, "sourceItems", sourceItems, "fixtureOnly", true));
        capture.game.setVarbit(4150, 2);
        UiModeCapture.load(hud, 12);
        capture.game.runScript(907, 161 << 16, 1130);
        frame(capture, hud, "bounded-bank-selected-tab", 12,
            OriginalCapture.map("selectedTab", 2, "tabSizes", new int[]{4,4}, "sourceItems", sourceItems, "fixtureOnly", true));
        bh.aq(95, 0, 14760, 0);
        capture.game.setVarbit(4150, 1);
        capture.game.setVarbit(3958, 1);
        capture.game.setVarbit(3959, 1);
        capture.game.setVarbit(3755, 1);
        UiModeCapture.load(hud, 12);
        UiModeCapture.op(capture, capture.game.getWidget(12, 31), 1);
        capture.game.runScript(907, 161 << 16, 1130);
        frame(capture, hud, "bounded-bank-placeholder-selected", 12,
            OriginalCapture.map("selectedTab", 1, "tabSizes", new int[]{4,4}, "placeholderSlot", 0, "sourceItem", 1265, "sourcePlaceholder", 14760,
                "notes", true, "insert", true, "placeholders", true, "amount", 5, "fixtureOnly", true));
        capture.game.closeInterface(main, true);
        capture.game.closeInterface(side, true);
        capture.game.runScript(907, 161 << 16, 1130);
    }

    static void hud(OriginalCapture capture, HudCapture hud) throws Exception
    {
        for (var node : new ArrayList<>(hud.sideNodes.values())) capture.game.closeInterface(node, true);
        hud.sideNodes.clear(); hud.enabledTabs.clear();
        for (int tab : new int[]{10,11})
        {
            hud.sideNodes.put(tab, capture.game.openInterface(161 << 16 | 76 + tab, HudCapture.TAB_GROUPS[tab], 1));
            hud.enabledTabs.add(tab);
        }
        capture.game.setVarbit(3756, 0);
        capture.game.runScript(907, 161 << 16, 1130);
        UiModeCapture.tab(hud, 11);
        frame(capture, hud, "bounded-hud-hidden", 116,
            OriginalCapture.map("introducedTabs", new int[]{10,11}, "activeTab", 11, "fixtureOnly", true));
        for (int tab = 0; tab <= 6; tab++)
        {
            hud.sideNodes.put(tab, capture.game.openInterface(161 << 16 | 76 + tab, HudCapture.TAB_GROUPS[tab], 1));
            hud.enabledTabs.add(tab);
        }
        bh.aq(93, 9, -1, 0); bh.aq(93, 10, -1, 0);
        UiModeCapture.load(hud, 218);
        UiModeCapture.tab(hud, 6);
        frame(capture, hud, "bounded-hud-locked", 218,
            OriginalCapture.map("introducedTabs", new int[]{0,1,2,3,4,5,6,10,11}, "activeTab", 6,
                "sourceMissingRuneSlots", new int[]{9,10}, "fixtureOnly", true));
        capture.game.setVarbit(3756, 4);
        UiModeCapture.tab(hud, 11);
        capture.game.runScript(907, 161 << 16, 1130);
        frame(capture, hud, "bounded-hud-highlight", 116,
            OriginalCapture.map("introducedTabs", new int[]{0,1,2,3,4,5,6,10,11}, "activeTab", 11,
                "flashsideVarbit3756", 4, "highlightTab", 3, "fixtureOnly", true));
        capture.game.setVarbit(3756, 0);
        for (int tab = 7; tab < 14; tab++) if (!hud.sideNodes.containsKey(tab))
        {
            hud.sideNodes.put(tab, capture.game.openInterface(161 << 16 | 76 + tab, HudCapture.TAB_GROUPS[tab], 1));
            hud.enabledTabs.add(tab);
        }
        UiModeCapture.tab(hud, 3);
    }

    static void levelUp(OriginalCapture capture, HudCapture hud) throws Exception
    {
        capture.game.addChatMessage(net.runelite.api.ChatMessageType.GAMEMESSAGE, "", "Source-only level-up chat notice.", null);
        capture.game.setVarcIntValue(1112, -1);
        capture.game.runScript(216);
        capture.game.runScript(907, 161 << 16, 1130);
        boolean found = false;
        var chatWidgets = new ArrayList<net.runelite.api.widgets.Widget>();
        var visited = new java.util.IdentityHashMap<net.runelite.api.widgets.Widget, Boolean>();
        for (lw widget : hud.widgets.ax[162]) UiAssetExport.visit(widget, visited, chatWidgets);
        for (var widget : chatWidgets)
            if (widget != null && widget.getText().contains("Source-only level-up chat notice.")) found = true;
        if (!found) throw new IllegalStateException("Original chat rebuild did not display the supplied source-only notice.");
        frame(capture, hud, "bounded-levelup-chat", 162,
            OriginalCapture.map("chatType", "GAMEMESSAGE", "text", "Source-only level-up chat notice.",
                "scope", "Native chat styling only; text is an explicit fixture, not a fabricated canonical level-up message."));
        var popup = capture.game.openInterface(162 << 16 | 567, 233, 0);
        capture.game.getWidget(233, 1).setText("Source-only level-up title.");
        capture.game.getWidget(233, 2).setText("Source-only level-up detail.");
        capture.game.getWidget(233, 3).setText("Click here to continue");
        capture.game.setVarcIntValue(1112, -1);
        capture.game.runScript(216);
        capture.game.runScript(907, 161 << 16, 1130);
        frame(capture, hud, "bounded-levelup-popup", 233,
            OriginalCapture.map("gamevalName", "levelup_display", "parent", 162 << 16 | 567,
                "title", "Source-only level-up title.", "detail", "Source-only level-up detail.",
                "scope", "Original widget233 source style, not an invented canonical ClubScape interface ID."));
        capture.game.closeInterface(popup, true);
    }

    static void run(OriginalCapture capture, HudCapture hud) throws Exception
    {
        settingsDefinitions(capture);
        settings(capture, hud);
        bank(capture, hud);
        hud(capture, hud);
        levelUp(capture, hud);
        if (records.size() != LIMIT) throw new IllegalStateException("The bounded comparison state inventory is incomplete");
        Files.writeString(capture.output.resolve("bounded-ui-inputs.json"),
            OriginalCapture.JSON.toJson(OriginalCapture.map("limit", LIMIT, "states", records)));
    }
}
