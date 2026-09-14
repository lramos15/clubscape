import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import net.runelite.api.Skill;
import net.runelite.api.widgets.Widget;
import net.runelite.api.IndexedSprite;
import net.runelite.cache.definitions.SpriteDefinition;
import net.runelite.cache.definitions.loaders.SpriteLoader;

/** Drives actual source widget loading, CS2 events, layout and rendering in a controlled offline host. */
public final class HudCapture
{
    final OriginalCapture capture;
    final vv widgets;
    qn context;
    final List<Object> events = new ArrayList<>();
    final java.util.Map<Integer, net.runelite.api.WidgetNode> sideNodes = new java.util.LinkedHashMap<>();
    final java.util.Set<Integer> enabledTabs = new java.util.TreeSet<>();
    String fixtureFamily = "all-unlocked";
    static final int[] TAB_GROUPS = {593, 320, 399, 149, 387, 541, 218, 707, 109, 429, 182, 116, 216, 239};

    HudCapture(OriginalCapture capture) throws Exception
    {
        this.capture = capture;
        int[] indexes = {index("ag"), index("aa"), 8, index("ad"), index("an")};
        System.out.println("NATIVE_WIDGET_ARCHIVES " + Arrays.toString(indexes));
        va[] inputs = new va[indexes.length];
        for (int i = 0; i < indexes.length; i++)
        {
            if (i == 4 && capture.cache.store.findIndex(indexes[i]).getArchives().isEmpty())
            {
                System.out.println("NATIVE_OPTIONAL_WIDGET_ARCHIVE " + indexes[i] + " has zero source groups; optional reference remains null.");
                continue;
            }
            inputs[i] = capture.cache.archive(indexes[i]);
        }
        widgets = new vv(inputs[0], inputs[1], inputs[2], inputs[3], inputs[4]);
        WorldCapture.staticField(wk.class, "cy", vv.class, widgets);
        WorldCapture.staticField(wn.class, "hs", vp.class, capture.cache.archive(12));
        WorldCapture.staticField(lh.class, "aa", va.class, capture.cache.archive(2));
        WorldCapture.staticField(bf.class, "hu", vp.class, capture.cache.archive(2));
        WorldCapture.staticField(pa.class, "ae", va.class, capture.cache.archive(2));
        WorldCapture.staticField(pn.class, "ax", va.class, capture.cache.archive(2));
        WorldCapture.staticField(zw.class, "ae", va.class, capture.cache.archive(2));
        WorldCapture.staticField(pr.class, "af", va.class, capture.cache.archive(2));
        WorldCapture.staticField(lp.class, "aw", va.class, capture.cache.archive(2));
        WorldCapture.staticField(ox.class, "ak", va.class, capture.cache.archive(7));
        int kitCount = Arrays.stream(capture.cache.archive(2).getFileIds(3)).max().orElseThrow() + 1;
        WorldCapture.logicalInt(null, ey.class, "ar", -1529735849, kitCount);
        WorldCapture.staticField(eb.class, "pw", vb.class, new vb());
        WorldCapture.staticField(sx.class, "et", ds.class, new FixtureVarcs());
        WorldCapture.staticField(kf.class, "kw", dm.class, new dm());
        WorldCapture.staticField(qq.class, "fx", bw.class, new bw(aao.az));
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("ai") && field.getType() == dj.class)
            {
                field.setAccessible(true);
                dj writer = (dj) field.get(null);
                if (writer.ap != null) throw new IllegalStateException("HUD fixture must not have a network transport");
                writer.ab = new yt(new int[4]);
            }
        }
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("wg") && field.getType() == java.util.concurrent.ScheduledExecutorService.class)
            {
                field.setAccessible(true);
                field.set(capture.game, new FixturePreferenceWrites());
            }
        }
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("ov") && field.getType() == vp[].class)
            {
                field.setAccessible(true);
                vp[] slots = (vp[]) field.get(null);
                for (int id : new int[]{3, 12, 17}) slots[id] = capture.cache.archive(id);
            }
        }
        WorldCapture.logicalInt(null, sa.class, "qy", 773246731, 1920);
        WorldCapture.logicalInt(null, eu.class, "qx", 8379747, 1080);
        WorldCapture.staticField(client.class, "ez", boolean.class, true);
        WorldCapture.logicalInt(null, client.class, "ci", -44590225, 30);
        WorldCapture.logicalInt(widgets, vv.class, "ap", -77072447, 161);
        int maximumVarp = 0;
        vp configs = capture.cache.archive(2);
        for (int id : configs.getFileIds(14))
        {
            net.runelite.api.VarbitComposition varbit = new pt(new xy(configs.loadData(14, id)));
            maximumVarp = Math.max(maximumVarp, varbit.getIndex());
        }
        WorldCapture.staticField(lb.class, "af", int[].class, new int[maximumVarp + 1]);
        for (var method : client.class.getDeclaredMethods())
        {
            if (method.getName().equals("al") && method.getReturnType() == void.class && method.getParameterCount() == 0
                && Modifier.isStatic(method.getModifiers()))
            {
                method.setAccessible(true);
                method.invoke(null);
            }
        }
        Arrays.fill(capture.game.getRealSkillLevels(), 1);
        Arrays.fill(capture.game.getBoostedSkillLevels(), 1);
        capture.game.getRealSkillLevels()[Skill.HITPOINTS.ordinal()] = 10;
        capture.game.getBoostedSkillLevels()[Skill.HITPOINTS.ordinal()] = 10;
        capture.game.getSkillExperiences()[Skill.HITPOINTS.ordinal()] = 1154;
        aam graphics = new aam();
        graphics.az(capture.cache.archive(17), -243617527);
        WorldCapture.staticField(ov.class, "lf", aam.class, graphics);
        WorldCapture.staticField(hc.class, "hv", vp.class, capture.cache.archive(8));
        WorldCapture.staticField(Class.forName("yo"), "ir", vp.class, capture.cache.archive(13));
        WorldCapture.staticField(be.class, "in", vp.class, capture.cache.archive(21));
        WorldCapture.staticField(ze.class, "ab", va.class, capture.cache.archive(2));
        WorldCapture.staticField(zu.class, "ae", va.class, capture.cache.archive(2));
        int compass = graphics.ao * -92792757;
        WorldCapture.staticField(gg.class, "mf", ym.class, (ym) capture.game.getSprites(capture.cache.archive(8), compass, 0)[0]);
        WorldCapture.staticField(oy.class, "aq", yz[].class, indexed(graphics.ap * -876542085));
        WorldCapture.staticField(pe.class, "aj", ym[].class, capture.game.getSprites(capture.cache.archive(8), graphics.ay * -1460057021, 0));
    }

    private int index(String member) throws Exception
    {
        Field field = um.class.getDeclaredField(member);
        field.setAccessible(true);
        Object constant = field.get(null);
        Field number = um.class.getDeclaredField("bn");
        number.setAccessible(true);
        return number.getInt(constant) * 1060637953;
    }

    private yz[] indexed(int group) throws Exception
    {
        SpriteDefinition[] source = new SpriteLoader().load(group, capture.cache.archive(8).loadData(group, 0));
        yz[] result = new yz[source.length];
        for (int i = 0; i < result.length; i++)
        {
            IndexedSprite sprite = capture.game.createIndexedSprite();
            sprite.setPixels(source[i].pixelIdx);
            sprite.setPalette(source[i].palette);
            sprite.setWidth(source[i].getWidth());
            sprite.setHeight(source[i].getHeight());
            sprite.setOriginalWidth(source[i].getMaxWidth());
            sprite.setOriginalHeight(source[i].getMaxHeight());
            sprite.setOffsetX(source[i].getOffsetX());
            sprite.setOffsetY(source[i].getOffsetY());
            result[i] = (yz) sprite;
        }
        return result;
    }

    private void load(int group, boolean onLoad) throws Exception
    {
        if (!ly.zt(widgets, group, 870024905)) throw new IllegalStateException("Native widget group unavailable " + group);
        System.out.println("NATIVE_WIDGET_GROUP " + group + " count=" + widgets.ax[group].length);
        if (onLoad)
        {
            for (lw nativeWidget : widgets.ax[group])
            {
                if (nativeWidget == null) continue;
                Widget widget = nativeWidget;
                Object[] script = widget.getOnLoadListener();
                if (script == null) continue;
                System.out.println("NATIVE_ONLOAD widget=" + widget.getId() + " script=" + Arrays.toString(script));
                capture.game.createScriptEventBuilder(script).setSource(widget).build().run();
                events.add(OriginalCapture.map("widget_id", widget.getId(), "arguments", script));
            }
        }
    }

    private void player() throws Exception
    {
        ct player = new ct(0);
        WorldCapture.logicalInt(player, dh.class, "bb", -1547553299, (3222 - 3168) * 128 + 64);
        WorldCapture.logicalInt(player, dh.class, "bi", -1272026483, (3218 - 3168) * 128 + 64);
        WorldCapture.logicalInt(player, ct.class, "ac", 810892507, 3);
        for (Field field : ct.class.getDeclaredFields())
        {
            if (field.getName().equals("ae") && field.getType() == aae.class)
            {
                field.setAccessible(true);
                field.set(player, new aae("Reference"));
            }
            if (field.getName().equals("ab") && field.getType() == lc.class)
            {
                field.setAccessible(true);
                lc appearance = new lc();
                appearance.ae(null, null, null, false, new int[5], 0, -1, -1, -1963864182);
                field.set(player, appearance);
            }
        }
        WorldCapture.staticField(dc.class, "cb", ct.class, player);
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("dr") && field.getType() == cl.class)
            {
                field.setAccessible(true);
                Object registry = field.get(null);
                WorldCapture.logicalInt(registry, cl.class, "ax", -1688595207, 1);
                for (Field table : cl.class.getDeclaredFields())
                {
                    if (table.getName().equals("as") && table.getType() == dz.class)
                    {
                        table.setAccessible(true);
                        table.set(registry, is.dk);
                    }
                    if (table.getName().equals("az") && table.getType() == yn.class)
                    {
                        table.setAccessible(true);
                        ((yn) table.get(registry)).af(is.dk, 0L);
                    }
                }
            }
        }
        is.dk.aq.af(player, 0L);
        WorldCapture.logicalInt(null, client.class, "da", -2034209657, 0);
        WorldCapture.logicalInt(null, client.class, "dj", -2130951373, 0);
        if (mb.ev(2006617018) != player) throw new IllegalStateException("Original local-player registry binding failed");
        capture.game.getLocalPlayer().setIdlePoseAnimation(808);
        capture.game.getLocalPlayer().setPoseAnimation(808);
        capture.game.getLocalPlayer().setWalkAnimation(819);
        System.out.println("CONTROLLED_NATIVE_PLAYER name=Reference combat=3 local=" + capture.game.getLocalPlayer().getLocalLocation());
    }

    private void inventory()
    {
        int[] items = {1265, 1351, 590, 303, 317, 315, 1511, 1925, 1931, 556, 558, 995};
        for (int i = 0; i < 28; i++) bh.aq(93, i, i < items.length ? items[i] : -1, i < items.length ? (i >= 9 ? 25 : 1) : 0);
        for (int i = 0; i < 14; i++) bh.aq(94, i, -1, 0);
        bh.aq(94, 3, 1277, 1);
        bh.aq(94, 5, 1171, 1);
        bh.aq(94, 13, 882, 25);
        if (capture.game.getItemContainer(93).getItems()[0].getId() != 1265)
        {
            throw new IllegalStateException("Native fixture container mutation failed");
        }
    }

    private void minimap() throws Exception
    {
        for (var method : client.class.getDeclaredMethods())
        {
            if (method.getName().equals("gq") && method.getReturnType() == long[].class && method.getParameterCount() == 0)
            {
                method.setAccessible(true);
                WorldCapture.staticField(client.class, "zw", long[].class, method.invoke(null));
            }
        }
        ym map = new ym(512, 512);
        client.bm(is.dk, map, 4.0, 0, 0, 0, 48, 48);
        if (Arrays.stream(map.getPixels()).filter(pixel -> (pixel & 0xffffff) > 1).count() < 10000)
        {
            throw new IllegalStateException("Native minimap source rendering was empty");
        }
        WorldCapture.staticField(rd.class, "ax", ym.class, map);
        WorldCapture.staticField(client.class, "ax", boolean.class, false);
        Object originalTileDrawer = null;
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("jr") && field.getType() == rl7.class)
            {
                field.setAccessible(true);
                originalTileDrawer = field.get(null);
            }
        }
        if (originalTileDrawer == null) throw new IllegalStateException("Missing native minimap tile renderer");
        WorldCapture.staticField(client.class, "sg", rl7.class, originalTileDrawer);
        WorldCapture.staticField(client.class, "hn", double.class, 4.0);
        System.out.println("NATIVE_MINIMAP generated512 source-scene scale4 at48,48");
    }

    private void selectTab(int tab) throws Exception
    {
        int component = tab < 7 ? 59 + tab : 43 + tab - 7;
        Widget stone = capture.game.getWidget(161, component);
        Object[] script = stone.getOnOpListener();
        if (script == null) throw new IllegalStateException("Native side-tab listener unavailable for " + tab);
        capture.game.createScriptEventBuilder(script).setSource(stone).setOp(1).build().run();
        capture.game.runScript(907, 161 << 16, 1130);
        events.add(OriginalCapture.map("operation", "native tab click", "tab", tab, "source_widget", stone.getId(), "arguments", script));
    }

    private void widgetState(Widget widget, java.util.Set<Widget> visited, List<Object> state)
    {
        if (widget == null || !visited.add(widget)) return;
        if (!widget.isHidden() && widget.getWidth() > 0 && widget.getHeight() > 0)
        {
            state.add(OriginalCapture.map("id", widget.getId(), "index", widget.getIndex(), "parent", widget.getParentId(),
                "x", widget.getCanvasLocation().getX(), "y", widget.getCanvasLocation().getY(),
                "width", widget.getWidth(), "height", widget.getHeight(), "type", widget.getType(),
                "text", widget.getText(), "sprite", widget.getSpriteId(), "item", widget.getItemId(),
                "item_quantity", widget.getItemQuantity()));
        }
        Widget[] children = widget.getChildren();
        if (children != null) for (Widget child : children) widgetState(child, visited, state);
    }

    private Object region(String name, Widget widget, int[] pixels) throws Exception
    {
        var location = widget.getCanvasLocation();
        int x0 = Math.max(0, location.getX()), y0 = Math.max(0, location.getY());
        int x1 = Math.min(1920, location.getX() + widget.getWidth());
        int y1 = Math.min(1080, location.getY() + widget.getHeight());
        if (x1 <= x0 || y1 <= y0) throw new IllegalStateException("Native UI region has no visible bounds: " + name);
        java.nio.ByteBuffer data = java.nio.ByteBuffer.allocate((x1 - x0) * (y1 - y0) * 4);
        java.util.Set<Integer> colors = new java.util.HashSet<>();
        int nonblack = 0;
        for (int y = y0; y < y1; y++)
        {
            for (int x = x0; x < x1; x++)
            {
                int rgb = pixels[y * 1920 + x] & 0xffffff;
                colors.add(rgb);
                if (rgb != 0) nonblack++;
                data.putInt(rgb | 0xff000000);
            }
        }
        return OriginalCapture.map("name", name, "source_widget", widget.getId(),
            "bounds", new int[]{x0, y0, x1 - x0, y1 - y0}, "colors", colors.size(), "nonblack_pixels", nonblack,
            "argb32_be_sha256", OriginalCapture.hash(data.array()));
    }

    private void frame(String name, int activeGroup) throws Exception
    {
        int[] pixels = capture.target(1920, 1080, 0, 512);
        gb.co.as(widgets, 1920, 1080, 1, 0, context, (byte) 1);
        qi.ck.az(1920, 1080, widgets, 1, 2, -293044276);
        List<Object> state = new ArrayList<>();
        java.util.Set<Widget> visited = java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
        for (lw[] group : widgets.ax)
        {
            if (group == null) continue;
            for (lw nativeWidget : group)
            {
                if (nativeWidget == null) continue;
                widgetState(nativeWidget, visited, state);
            }
        }
        List<Object> regions = List.of(region("minimap", capture.game.getWidget(161, 95), pixels),
            region("chat", capture.game.getWidget(161, 96), pixels),
            region("sidebar", capture.game.getWidget(161, 97), pixels),
            region("active-panel", capture.game.getWidget(activeGroup, 0), pixels));
        java.util.Map<Integer, Integer> varps = new java.util.TreeMap<>();
        int[] currentVarps = capture.game.getVarps();
        for (int i = 0; i < currentVarps.length; i++) if (currentVarps[i] != 0) varps.put(i, currentVarps[i]);
        List<Object> containers = new ArrayList<>();
        for (int id : new int[]{93, 94, 95, 3})
        {
            var container = capture.game.getItemContainer(id);
            if (container != null) containers.add(OriginalCapture.map("id", id, "items", container.getItems()));
        }
        List<Object> links = new ArrayList<>();
        for (var link : capture.game.getComponentTable())
        {
            links.add(OriginalCapture.map("parent_component", link.getHash(), "interface_group", link.getId()));
        }
        int preferenceSaveRequests = 0;
        for (Field field : client.class.getDeclaredFields())
        {
            field.setAccessible(true);
            if (field.getName().equals("ai") && field.getType() == dj.class)
            {
                if (((dj) field.get(null)).ap != null) throw new IllegalStateException("Unexpected network transport in native HUD fixture");
            }
            if (field.getName().equals("wg") && field.getType() == java.util.concurrent.ScheduledExecutorService.class)
            {
                preferenceSaveRequests = ((FixturePreferenceWrites) field.get(capture.game)).requests.size();
            }
        }
        capture.save("hud/" + name, pixels, 1920, 1080, 0, "original-runtime-hud-fixture",
            OriginalCapture.map("root_interface", 161, "active_interface", activeGroup, "native_draw", "gp.az",
                "events", List.copyOf(events), "visible_widgets", state, "native_ui_regions", regions, "component_links", links,
                "widget_state_scope", "Native non-hidden, positive-size widgets; descendants can still be clipped by native scrolling"),
            OriginalCapture.map("layout", "Original Resizable-Classic group161", "resized", true, "canvas", new int[]{1920, 1080},
                "controlled_state", true, "authenticated", false, "display_name", "Reference",
                "ui_family", fixtureFamily, "enabled_tab_slots", new ArrayList<>(enabledTabs),
                "source_state_scope", "Controlled source UI attachments/inputs, not authenticated tutorial progression",
                "nonzero_varps", varps, "containers", containers,
                "player_appearance_kits", capture.game.getLocalPlayer().getPlayerComposition().getEquipmentIds(),
                "network_transport_connected", false, "login_handler_invoked", false,
                "native_ui_requests", "Locally queued only with a zero-key fixture ISAAC; no packet writer flush/network pump",
                "preference_save_requests_recorded", preferenceSaveRequests,
                "main_varps", "source-zero defaults plus explicitly declared fixture/UI settings"), 10000);
    }

    private void bank() throws Exception
    {
        int[] items = {995, 436, 438, 2349, 1265, 1351, 1927, 1933, 1944, 1947, 315, 1511};
        for (int i = 0; i < items.length; i++) bh.aq(95, i, items[i], i == 0 ? 1000 : i + 1);
        selectTab(3);
        var main = capture.game.openInterface(161 << 16 | 16, 12, 0);
        var side = capture.game.openInterface(161 << 16 | 74, 15, 1);
        capture.game.runScript(907, 161 << 16, 1130);
        frame("native-bank", 12);
        capture.game.closeInterface(main, true);
        capture.game.closeInterface(side, true);
        capture.game.runScript(907, 161 << 16, 1130);
    }

    private void shop() throws Exception
    {
        int[] items = {1931, 1925, 1735, 560, 590, 1755, 2347, 550, 555, 560, 1265, 1351};
        int size = new net.runelite.cache.definitions.loaders.InventoryLoader()
            .load(3, capture.cache.archive(2).loadData(5, 3)).getSize();
        for (int i = 0; i < size; i++) bh.aq(3, i, i < items.length ? items[i] : -1, i < items.length ? 10 : 0);
        selectTab(3);
        var main = capture.game.openInterface(161 << 16 | 16, 300, 0);
        var side = capture.game.openInterface(161 << 16 | 74, 301, 1);
        capture.game.runScript(1074, 3, -1, 0, 1, "General Store");
        capture.game.runScript(6007, 301 << 16, 1);
        capture.game.runScript(907, 161 << 16, 1130);
        frame("native-shop", 300);
        capture.game.closeInterface(main, true);
        capture.game.closeInterface(side, true);
        capture.game.runScript(907, 161 << 16, 1130);
    }

    private void dialogue(int npc, String text, String name) throws Exception
    {
        var node = capture.game.openInterface(162 << 16 | 567, 231, 0);
        capture.game.getWidget(231, 2).setModelType(2).setModelId(npc);
        capture.game.getWidget(231, 4).setText(capture.game.getNpcDefinition(npc).getName());
        capture.game.getWidget(231, 6).setText(text);
        capture.game.runScript(216);
        capture.game.runScript(907, 161 << 16, 1130);
        frame(name, 231);
        capture.game.closeInterface(node, true);
        capture.game.runScript(216);
        capture.game.runScript(907, 161 << 16, 1130);
    }

    private void unlockFamilies() throws Exception
    {
        String[] names = {"guide", "survival", "quest-guide", "combat", "prayer", "magic"};
        int[] npcs = {3308, 3306, 3312, 3307, 3319, 3309};
        int[] selected = {11, 3, 2, 0, 5, 6};
        int[][] tabs = {
            {10, 11},
            {10, 11, 1, 3},
            {10, 11, 1, 2, 3},
            {10, 11, 0, 1, 2, 3, 4},
            {10, 11, 0, 1, 2, 3, 4, 5},
            {10, 11, 0, 1, 2, 3, 4, 5, 6}
        };
        for (int stage = 0; stage < names.length; stage++)
        {
            fixtureFamily = "tutorial-interface-family-" + names[stage];
            for (var node : new ArrayList<>(sideNodes.values())) capture.game.closeInterface(node, true);
            sideNodes.clear();
            enabledTabs.clear();
            for (int tab : tabs[stage])
            {
                sideNodes.put(tab, capture.game.openInterface(161 << 16 | (76 + tab), TAB_GROUPS[tab], 1));
                enabledTabs.add(tab);
            }
            WorldCapture.logicalInt(null, ba.class, "ao", -782895767, stage == 0 ? 2 : 0);
            capture.game.runScript(907, 161 << 16, 1130);
            selectTab(selected[stage]);
            dialogue(npcs[stage], "Controlled " + names[stage] + " interface-family fixture.<br>Native widgets and scripts; not an observed tutorial dialogue.",
                "family-" + names[stage]);
        }
    }

    void run() throws Exception
    {
        int[] questTotals = HudSourceCatalog.write(capture.cache, capture.output);
        new WorldCapture(capture).prepareForHud();
        client.gc(-1);
        WorldCapture.staticField(lb.class, "az", int[].class, capture.game.getVarps().clone());
        player();
        inventory();
        capture.game.setVarbit(4609, 1);
        capture.game.setVarbit(5605, 1);
        capture.game.setVarbit(8119, 1);
        capture.game.setVarbit(357, 17);
        capture.game.setVarbit(11877, questTotals[0]);
        capture.game.setVarbit(1782, questTotals[1]);
        capture.game.setVarbit(6347, 0);
        WorldCapture.logicalInt(null, ba.class, "ao", -782895767, 0);
        WorldCapture.logicalInt(null, client.class, "np", 2106329293, (3222 - 3168) * 128 + 64);
        WorldCapture.logicalInt(null, client.class, "nq", -2126074583, (3218 - 3168) * 128 + 64);
        minimap();
        int[] pixels = capture.target(1920, 1080, 0, 512);
        load(161, false);
        context = null;
        for (Field field : client.class.getDeclaredFields())
        {
            if (field.getName().equals("ca") && field.getType() == qn.class)
            {
                field.setAccessible(true);
                context = (qn) field.get(null);
            }

        }
        if (context == null) throw new IllegalStateException("Missing native widget event context");
        cn.ae(161, 1920, 1080, false, widgets, context, (short) 217);
        load(161, true);
        for (int[] binding : new int[][]{{96, 162}, {33, 160}, {76, 593}, {77, 320}, {78, 399},
            {79, 149}, {80, 387}, {81, 541}, {82, 218}, {83, 707}, {84, 109}, {85, 429},
            {86, 182}, {87, 116}, {88, 216}, {89, 239}})
        {
            System.out.println("NATIVE_ATTACH component=" + binding[0] + " group=" + binding[1]);
            var node = capture.game.openInterface(161 << 16 | binding[0], binding[1], 1);
            if (binding[0] >= 76 && binding[0] <= 89)
            {
                sideNodes.put(binding[0] - 76, node);
                enabledTabs.add(binding[0] - 76);
            }
        }
        String[] names = {"combat", "skills", "quest-list", "inventory", "equipment", "prayer", "magic"};
        int[] groups = {593, 320, 399, 149, 387, 541, 218};
        for (int tab : new int[]{3, 4, 1, 0, 5, 6, 2})
        {
            selectTab(tab);
            frame("native-" + names[tab], groups[tab]);
        }
        bank();
        shop();
        dialogue(3308, "Welcome! This controlled reference state exercises the original dialogue interface.", "native-guide-dialogue");
        unlockFamilies();
    }
}
