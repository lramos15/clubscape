import java.util.ArrayList;
import java.util.List;
import net.runelite.api.IndexedSprite;
import net.runelite.cache.definitions.SpriteDefinition;
import net.runelite.cache.definitions.loaders.SpriteLoader;

/** Read-only calls to the original title/login painter; no handlers or authentication are invoked. */
public final class TitleCapture
{
    final OriginalCapture capture;
    final List<Object> inputs = new ArrayList<>();

    TitleCapture(OriginalCapture capture) { this.capture = capture; }

    private yz indexed(String name) throws Exception
    {
        int group = capture.cache.store.findIndex(8).findArchiveByName(name).getArchiveId();
        inputs.add(OriginalCapture.map("archive", 8, "group", group, "file", 0, "name", name));
        return su.ag(capture.cache.archive(8), name, "", -281278256);
    }

    private yz[] indexedFrames(String name) throws Exception
    {
        int group = capture.cache.store.findIndex(8).findArchiveByName(name).getArchiveId();
        byte[] bytes = capture.cache.archive(8).loadData(group, 0);
        inputs.add(OriginalCapture.map("archive", 8, "group", group, "file", 0, "name", name,
            "sha256", OriginalCapture.hash(bytes)));
        SpriteDefinition[] definitions = new SpriteLoader().load(group, bytes);
        yz[] result = new yz[definitions.length];
        for (int i = 0; i < result.length; i++)
        {
            SpriteDefinition definition = definitions[i];
            IndexedSprite sprite = capture.game.createIndexedSprite();
            sprite.setPixels(definition.pixelIdx);
            sprite.setPalette(definition.palette);
            sprite.setWidth(definition.getWidth());
            sprite.setHeight(definition.getHeight());
            sprite.setOffsetX(definition.getOffsetX());
            sprite.setOffsetY(definition.getOffsetY());
            sprite.setOriginalWidth(definition.getMaxWidth());
            sprite.setOriginalHeight(definition.getMaxHeight());
            result[i] = (yz) sprite;
        }
        return result;
    }

    private void asset(Class<?> owner, String field, String name) throws Exception
    {
        WorldCapture.staticField(owner, field, yz.class, indexed(name));
    }

    void run() throws Exception
    {
        WorldCapture.logicalInt(null, sa.class, "qy", 773246731, 1920);
        WorldCapture.logicalInt(null, eu.class, "qx", 8379747, 1080);
        WorldCapture.staticField(hc.class, "hv", vp.class, capture.cache.archive(8));
        int titleGroup = capture.cache.store.findIndex(10).findArchiveByName("title.jpg").getArchiveId();
        byte[] jpeg = capture.cache.archive(10).loadData(titleGroup, 0);
        ym left = it.az(jpeg, 1951476339);
        WorldCapture.staticField(ni.class, "cy", ym.class, left);
        WorldCapture.staticField(fr.class, "co", ym.class, left.al());
        inputs.add(OriginalCapture.map("archive", 10, "group", titleGroup, "file", 0,
            "name", "title.jpg", "sha256", OriginalCapture.hash(jpeg), "mirror_method", "Original ym.al"));
        asset(fp.class, "ck", "logo");
        asset(jb.class, "cl", "titlebox");
        asset(ka.class, "cd", "titlebutton");
        asset(gq.class, "cv", "titlebutton_large");
        asset(qh.class, "cs", "play_now_text");
        asset(ck.class, "cc", "options_radio_buttons,0");
        asset(ek.class, "cn", "options_radio_buttons,2");
        asset(hc.class, "cf", "options_radio_buttons,4");
        asset(qn.class, "ch", "options_radio_buttons,6");
        WorldCapture.staticField(cg.class, "ca", yz[].class, indexedFrames("title_mute"));
        yz[] runes = indexedFrames("runes");
        WorldCapture.staticField(bm.class, "cq", yz[].class, runes);
        aam defaults = new aam();
        defaults.az(capture.cache.archive(17), -243617527);
        WorldCapture.staticField(ml.class, "bi", cs.class, new cs(runes, defaults.ac));
        WorldCapture.staticField(bb.class, "du", zv.class, capture.fonts.get(496));
        WorldCapture.staticField(qj.class, "dy", zv.class, capture.fonts.get(495));
        WorldCapture.staticField(vp.class, "di", zv.class, capture.fonts.get(494));
        WorldCapture.staticField(bf.class, "dt", boolean.class, false);
        for (int[] state : new int[][]{{0, 0}, {10, 0}, {10, 2}, {10, 3}, {10, 12}})
        {
            int index = state[1];
            WorldCapture.logicalInt(null, client.class, "ci", -44590225, state[0]);
            WorldCapture.logicalInt(null, bf.class, "cw", -47366135, index);
            int[] pixels = capture.target(1920, 1080, 0, 512);
            hs.ai(capture.fonts.get(496), capture.fonts.get(495), capture.fonts.get(494), -1492612357);
            capture.save("title/state-" + state[0] + "-login-index-" + index, pixels, 1920, 1080, 0, "original-runtime-title-state-fixture",
                OriginalCapture.map("source_inputs", inputs, "native_method", "hs.ai(zv,zv,zv,int)",
                    "native_argument_guard", -1492612357),
                OriginalCapture.map("canvas", new int[]{1920, 1080}, "game_state_value", state[0],
                    "login_index", index, "login_index_readback", capture.game.getLoginIndex(),
                    "cycle", 0, "font_ids_in_argument_order", new int[]{496, 495, 494},
                    "flame_palette", defaults.ac, "world_select_open", false, "username_password", "empty",
                    "state_classification", "Pure original painter with controlled logged-out state, not an observed login/EULA transition.",
                    "terms_accepted", false, "authenticated", false), 20000);
        }
    }
}
