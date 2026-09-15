import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.runelite.api.widgets.Widget;

/** Explicit original-runtime UI scenarios. Never a live account or gameplay fixture. */
public final class UiModeCapture
{
    static Object[] listener(Widget widget, String fieldName) throws Exception
    {
        for (var field : lw.class.getDeclaredFields())
            if (field.getName().equals(fieldName) && field.getType() == Object[].class)
            {
                field.setAccessible(true);
                return (Object[]) field.get(widget);
            }
        throw new IllegalStateException("Pinned source listener field missing: " + fieldName);
    }

    static void click(OriginalCapture capture, Widget widget) throws Exception
    {
        Object[] script = listener(widget, "ec");
        if (script == null) throw new IllegalStateException("Native click listener absent: " + widget.getId());
        capture.game.createScriptEventBuilder(script).setSource(widget).build().run();
    }

    static void op(OriginalCapture capture, Widget widget, int operation) throws Exception
    {
        Object[] script = widget.getOnOpListener();
        if (script == null) throw new IllegalStateException("Native operation listener absent: " + widget.getId());
        capture.game.createScriptEventBuilder(script).setSource(widget).setOp(operation).build().run();
    }

    static void host(HudCapture hud, String method, Class<?>[] types, Object... args) throws Exception
    {
        var target = HudCapture.class.getDeclaredMethod(method, types);
        target.setAccessible(true); target.invoke(hud, args);
    }

    static void frame(HudCapture hud, String name, int group) throws Exception
    {
        host(hud, "frame", new Class<?>[]{String.class, int.class}, name, group);
    }

    static void load(HudCapture hud, int group) throws Exception
    {
        host(hud, "load", new Class<?>[]{int.class, boolean.class}, group, true);
    }

    static void tab(HudCapture hud, int slot) throws Exception
    {
        host(hud, "selectTab", new Class<?>[]{int.class}, slot);
    }

    static void run(OriginalCapture capture, HudCapture hud) throws Exception
    {
        List<Object> scenarios = new ArrayList<>();
        so muted = (so) java.lang.reflect.Proxy.newProxyInstance(so.class.getClassLoader(), new Class<?>[]{so.class},
            (proxy, method, arguments) -> 0);
        WorldCapture.staticField(bi.class, "kv", sm.class, new sm(muted));
        scenarios.add(OriginalCapture.map("kind", "sound_boundary", "source", "ee.bt opcode3200 -> bi.kv/sm.tp",
            "volume", 0, "scope", "Original queue with a silent UI-only preference provider; no audio playback assertion."));
        for (int[] panel : new int[][]{{541, 5, 6}, {218, 6, 209}})
        {
            int group = panel[0];
            load(hud, group); tab(hud, panel[1]);
            frame(hud, "native-" + (group == 541 ? "prayer" : "magic") + "-filter-ready", group);
            Widget button = capture.game.getWidget(group, panel[2]);
            Object[] click = listener(button, "ec");
            scenarios.add(OriginalCapture.map("kind", "filter_open", "group", group, "widget", button.getId(),
                "sourceClick", click, "scope", "source UI only; no authority or gameplay assertions"));
            click(capture, button);
            frame(hud, "native-" + (group == 541 ? "prayer" : "magic") + "-filters", group);
            int[] bits = group == 541 ? new int[]{6574, 6575, 6576, 6577, 6578}
                : new int[]{6605, 6609, 6606, 6607, 6608, 12137, 6548};
            List<Integer> masks = new ArrayList<>();
            masks.add(0);
            for (int bit = 0; bit < bits.length; bit++) masks.add(1 << bit);
            masks.add((1 << bits.length) - 1);
            if (group == 218) { masks.add(72); masks.add(88); }
            for (int mask : masks)
            {
                for (int bit = 0; bit < bits.length; bit++) capture.game.setVarbit(bits[bit], mask >> bit & 1);
                load(hud, group); tab(hud, panel[1]);
                String base = "native-" + (group == 541 ? "prayer" : "magic") + "-mask-" + mask;
                frame(hud, base, group);
                button = capture.game.getWidget(group, panel[2]);
                click(capture, button);
                frame(hud, base + "-filters", group);
                scenarios.add(OriginalCapture.map("kind", "filter_preferences", "group", group,
                    "mask", mask, "varbits", bits, "closed", base, "open", base + "-filters",
                    "scope", "Explicit original UI preference inputs; not gameplay grants or observed journey state."));
            }
            for (int bit : bits) capture.game.setVarbit(bit, 0);
            load(hud, group);
        }
        tab(hud, 3);
        int[] sourceItems = {1265, 995, 315, 556, 558, 1277, 1171, 882, 1351, 303, 590, 1511, 1925, 1931, 436, 438};
        for (int inventory : new int[]{525, 636})
            for (int slot = 0; slot < 120; slot++)
                bh.aq(inventory, slot, slot < sourceItems.length ? sourceItems[slot] : -1,
                    slot < sourceItems.length ? (slot == 1 ? 1500 : slot == 3 ? 25 : slot == 7 ? 7 : 1) : 0);
        for (int[] values : new int[][]{{602, 34, 0, 0}, {602, 35, 0, 1},
            {669, 12345, -1, 0}, {669, 12345, 0, 42}, {669, 12345, 7, 42}, {669, 0, 7, 1}})
        {
            int group = values[0];
            for (int i = 0; i < 3; i++) capture.game.getVarps()[261 + i] = values[i + 1];
            var main = capture.game.openInterface(161 << 16 | 16, group, 0);
            capture.game.runScript(907, 161 << 16, 1130);
            String name = "native-retrieval-" + group + "-" + values[1] + "-" + values[2] + "-" + values[3];
            frame(hud, name, group);
            scenarios.add(OriginalCapture.map("kind", "retrieval_fields", "group", group, "case", name,
                "inventory", group == 602 ? 525 : 636, "sourceItems", sourceItems,
                "varp261", values[1], "varp262", values[2], "varp263", values[3],
                "scope", "Explicit native component-only inventory/coffer/selection/fee inputs; never production balances."));
            capture.game.closeInterface(main, true);
            capture.game.runScript(907, 161 << 16, 1130);
        }
        for (int group : new int[]{602, 669})
        {
            int inventory = group == 602 ? 525 : 636;
            for (int slot = 0; slot < 120; slot++)
                bh.aq(inventory, slot, slot < 80 ? sourceItems[slot % sourceItems.length] : -1,
                    slot < 80 ? (slot % sourceItems.length == 7 ? 7 : 1) : 0);
            capture.game.getVarps()[261] = group == 602 ? 34 : 12345;
            capture.game.getVarps()[262] = group == 602 ? 0 : 7;
            capture.game.getVarps()[263] = group == 602 ? 0 : 42;
            var main = capture.game.openInterface(161 << 16 | 16, group, 0);
            capture.game.runScript(907, 161 << 16, 1130);
            frame(hud, "native-retrieval-long-" + group, group);
            capture.game.getWidget(group, 3).setScrollY(60);
            capture.game.runScript(231, group << 16 | 4, group << 16 | 3);
            frame(hud, "native-retrieval-scroll-" + group, group);
            scenarios.add(OriginalCapture.map("kind", "retrieval_scroll", "group", group,
                "count", 80, "scrollY", 60, "case", "native-retrieval-scroll-" + group,
                "scope", "Explicit original component-only scrolling inventory, not production balances or progression."));
            capture.game.closeInterface(main, true);
            capture.game.runScript(907, 161 << 16, 1130);
        }
        Files.writeString(capture.output.resolve("mode-inputs.json"), OriginalCapture.JSON.toJson(scenarios));
    }
}
