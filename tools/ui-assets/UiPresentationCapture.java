import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Explicit source-only menu inputs; never a production account or gameplay implementation. */
public final class UiPresentationCapture
{
    static void production(OriginalCapture capture, int[] items, int amount) throws Exception
    {
        Object[] arguments = new Object[23];
        arguments[0] = 2046;
        arguments[1] = 0;
        arguments[2] = 28;
        List<String> names = new ArrayList<>();
        names.add("");
        for (int index = 0; index < 18; index++)
        {
            arguments[index + 3] = index < items.length ? items[index] : -1;
            if (index < items.length) names.add(capture.game.getItemDefinition(items[index]).getName());
        }
        arguments[21] = amount;
        arguments[22] = String.join("|", names);
        capture.game.runScript(arguments);
        capture.game.runScript(216);
        capture.game.runScript(907, 161 << 16, 1130);
    }

    static void run(OriginalCapture capture, HudCapture hud) throws Exception
    {
        List<Object> inputs = new ArrayList<>();
        UiModeCapture.tab(hud, 3);
        int[] published = OriginalCapture.JSON.fromJson(
            Files.readString(Path.of(System.getProperty("clubscape.ui.production"))), int[].class);
        var chat = capture.game.openInterface(162 << 16 | 567, 270, 0);
        for (int count = 1; count <= 18; count++)
        {
            int[] items = new int[count];
            for (int index = 0; index < count; index++) items[index] = published[index % published.length];
            production(capture, items, 1);
            String name = "native-production-choice-" + count;
            UiModeCapture.frame(hud, name, 270);
            inputs.add(OriginalCapture.map("case", name, "group", 270, "script", 2046,
                "operationType", 0, "maximumQuantity", 28, "selectedQuantity", 1, "sourceItems", items,
                "scope", "Explicit source-only display fields, not legal quantities or a gameplay menu."));
            if (count == 2)
            {
                var first = capture.game.getWidget(270, 15);
                capture.game.runScript(2049, 1, items[0], first.getId(), first.getWidth(), first.getHeight());
                UiModeCapture.frame(hud, "native-production-hover", 270);
                production(capture, items, 5);
                UiModeCapture.frame(hud, "native-production-amount-5", 270);
            }
            for (int item : published)
            {
                Arrays.fill(items, item);
                production(capture, items, 1);
                capture.target(1920, 1080, 0, 512);
                gb.co.as(hud.widgets, 1920, 1080, 1, 0, hud.context, (byte) 1);
                qi.ck.az(1920, 1080, hud.widgets, 1, 2, -293044276);
                UiAssetExport.staticModels(capture, hud);
                if (count > 10)
                {
                    var viewport = capture.game.getWidget(270, 13);
                    viewport.setScrollY(viewport.getScrollHeight() - viewport.getHeight());
                    capture.game.runScript(231, 270 << 16 | 33, viewport.getId());
                    gb.co.as(hud.widgets, 1920, 1080, 1, 0, hud.context, (byte) 1);
                    qi.ck.az(1920, 1080, hud.widgets, 1, 2, -293044276);
                    UiAssetExport.staticModels(capture, hud);
                    viewport.setScrollY(0);
                }
            }
        }
        capture.game.closeInterface(chat, true);
        capture.game.runScript(216);
        capture.game.runScript(907, 161 << 16, 1130);

        int oldMetal = capture.game.getVarbitValue(3216);
        capture.game.setVarbit(3216, 1);
        var smithing = capture.game.openInterface(161 << 16 | 16, 312, 0);
        capture.game.runScript(907, 161 << 16, 1130);
        UiModeCapture.frame(hud, "native-smithing-bronze", 312);
        inputs.add(OriginalCapture.map("case", "native-smithing-bronze", "group", 312,
            "varbit3216", 1, "sourceBar", 2349, "script", 430,
            "scope", "Source UI-only bronze table, not authoritative allowed recipes."));
        capture.game.closeInterface(smithing, true);
        capture.game.setVarbit(3216, oldMetal);

        int[] deathItems = {1265, 1351, 1205, 315, 556, 558, 995, 2309};
        for (int slot = 0; slot < 50; slot++)
        {
            bh.aq(584, slot, slot < deathItems.length ? deathItems[slot] : -1,
                slot < deathItems.length ? (slot == 6 ? 1500 : slot == 4 ? 25 : 1) : 0);
            bh.aq(468, slot, slot < deathItems.length ? (slot < 3 ? 323 : 367) : -1,
                slot < deathItems.length ? 1 : 0);
        }
        var death = capture.game.openInterface(161 << 16 | 16, 4, 0);
        capture.game.runScript(972, 0, 0, 0, 0, 0, -1, -1, -1, -1, "");
        capture.game.getWidget(4, 18).setText("View retrieval fees");
        capture.game.runScript(907, 161 << 16, 1130);
        UiModeCapture.frame(hud, "native-death-preview-populated", 4);
        inputs.add(OriginalCapture.map("case", "native-death-preview-populated", "group", 4, "script", 972,
            "intArguments", new int[]{0, 0, 0, 0, 0, -1, -1, -1, -1}, "textArgument", "",
            "sourceItems", deathItems, "itemContainer", 584, "classificationContainer", 468,
            "classificationMarkers", new int[]{323, 323, 323, 367, 367, 367, 367, 367}, "feePlusOne", 1,
            "explicitTextFields", OriginalCapture.map("4:18", "View retrieval fees"),
            "scope", "Explicit source-only death-preview inputs; no client death classification or fee calculation."));
        capture.game.closeInterface(death, true);
        capture.game.runScript(907, 161 << 16, 1130);

        var reward = capture.game.openInterface(161 << 16 | 16, 153, 0);
        String[] lines = {"Source-only award line", "2 x Shrimps", "1,234.5 Cooking XP", "1 Quest point"};
        capture.game.getWidget(153, 4).setText("Source fixture reward");
        for (int line = 0; line < 7; line++) capture.game.getWidget(153, 9 + line).setText(line < lines.length ? lines[line] : "");
        capture.game.getWidget(153, 6).setText("Total Quest Points: 7");
        capture.game.runScript(907, 161 << 16, 1130);
        UiModeCapture.frame(hud, "native-reward-fields", 153);
        inputs.add(OriginalCapture.map("case", "native-reward-fields", "group", 153,
            "title", "Source fixture reward", "lines", lines, "totalQuestPoints", 7,
            "scope", "Explicit synthetic text/item/XP fields in original widgets; not real journey rewards."));
        capture.game.closeInterface(reward, true);
        capture.game.runScript(907, 161 << 16, 1130);

        Files.writeString(capture.output.resolve("presentation-inputs.json"), OriginalCapture.JSON.toJson(inputs));
    }
}
