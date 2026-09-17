import com.google.gson.GsonBuilder;
import java.lang.reflect.Modifier;
import java.lang.reflect.Proxy;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import net.runelite.api.events.SoundEffectPlayed;
import net.runelite.api.hooks.Callbacks;

public final class NativeBindingProbe
{
    static final List<Object> callbackEvents = new ArrayList<>();
    static final List<Object> tests = new ArrayList<>();
    static final int[] volumes = {127, 127};
    static client sourceClient;

    static void require(boolean value, String message)
    {
        if (!value) throw new IllegalStateException(message);
    }

    static int encode(int value, int decoder)
    {
        return value * BigInteger.valueOf(Integer.toUnsignedLong(decoder))
            .modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
    }

    static Object allocate(Class<?> type) throws Exception
    {
        Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
        var field = unsafeClass.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return unsafeClass.getMethod("allocateInstance", Class.class).invoke(field.get(null), type);
    }

    static void setup(Path inputs) throws Exception
    {
        client instance = (client) allocate(client.class);
        sourceClient = instance;
        Callbacks callbacks = (Callbacks) Proxy.newProxyInstance(Callbacks.class.getClassLoader(),
            new Class<?>[]{Callbacks.class}, (proxy, method, args) ->
            {
                if (method.getName().equals("post"))
                {
                    Object event = args[args.length - 1];
                    if (event instanceof SoundEffectPlayed effect)
                        callbackEvents.add(BindingCache.map("event", "SoundEffectPlayed",
                            "sound_id", effect.getSoundId(), "delay", effect.getDelay()));
                    else callbackEvents.add(BindingCache.map("event", event.getClass().getSimpleName()));
                }
                Class<?> type = method.getReturnType();
                if (type == boolean.class) return false;
                if (type == int.class) return 0;
                if (type == long.class) return 0L;
                return null;
            });
        for (var field : client.class.getDeclaredFields())
        {
            if (field.getType() != Callbacks.class) continue;
            field.setAccessible(true);
            field.set(Modifier.isStatic(field.getModifiers()) ? null : instance, callbacks);
        }
        oe.cz = instance;
        ab.kn = new cy();
        qo.ki = new ao();
        tz.hq = SourceAudio.OfflineArchive.load(inputs, 4);
        lg.al = 22050 * (int) 3883844509L;
        kg.aj = true;
        so provider = (so) Proxy.newProxyInstance(so.class.getClassLoader(), new Class<?>[]{so.class},
            (proxy, method, args) ->
            {
                if (method.getName().equals("as")) return volumes[0];
                if (method.getName().equals("ag")) return volumes[1];
                throw new UnsupportedOperationException("Unselected volume-provider method " + method.getName());
            });
        bi.kv = new sm(provider);
        kf.kw = new dm();
        musicVolume(128);
        cb.hn = SourceAudio.OfflineArchive.load(inputs, 11);
    }

    static void musicVolume(int value)
    {
        kf.kw.ag = encode(value, -12931133);
        require(mh.gv((byte) 11) == value, "Native music-volume getter differs");
    }

    static void resetJingles()
    {
        np.ae.clear();
        np.ab.clear();
        np.ac.clear();
        np.aa.clear();
        client.ka = false;
    }

    static List<Integer> pending()
    {
        List<Integer> ids = new ArrayList<>();
        for (Object value : np.ae) ids.add(((nb) value).getArchiveId());
        return ids;
    }

    static void jingle(int id, int argument)
    {
        bb.ay[0] = id;
        bb.ay[1] = argument;
        dy.aq = 2 * 120041229;
        int result = ee.bt(3202, null, false, (byte) 0);
        require(result == 1 && dy.aq * -324749371 == 0, "Native jingle opcode stack contract differs");
    }

    static void effect(int id, int loops, int delay)
    {
        bb.ay[0] = id;
        bb.ay[1] = loops;
        bb.ay[2] = delay;
        dy.aq = 3 * 120041229;
        int result = ee.bt(3200, null, false, (byte) 0);
        require(result == 1 && dy.aq * -324749371 == 0, "Native SFX opcode stack contract differs");
    }

    static int soundCount()
    {
        return bi.kv.af * 2086456713;
    }

    static Map<String, Object> sound(int index)
    {
        sb value = bi.kv.ae[index];
        return BindingCache.map("sound_id", value.ax * encode(1, -1575487195),
            "world_view_id", value.ae * encode(1, 322732777),
            "packed_location", value.ab * encode(1, -643204909),
            "loops", value.as * encode(1, -1206170819),
            "delay", value.ag * encode(1, -509408507),
            "retain", value.af * encode(1, -1796103743), "native_boolean", value.ac);
    }

    static void run(Path inputs, Path output) throws Exception
    {
        setup(inputs);
        nu initialized = new nu(new ak());
        require(initialized.ac[9] == 0 && initialized.bx[9] == 0, "Unexpected uninitialized native program defaults");
        initialized.ap(9, 128, (short) -27396);
        require(initialized.ac[9] == 128 && initialized.aq[9] == 128 && initialized.bx[9] == 128,
            "Native startup percussion-bank assignment differs");
        tests.add(BindingCache.map("case", "native-startup-percussion-bank",
            "constructor_default_bank_channel9", 0, "source_startup_default_bank_channel9", initialized.ac[9],
            "source_startup_call", "dg.ay -> nu.ap(9,128,-27396)"));
        bi.kv.ag(0);
        effect(2393, 1, 7);
        require(soundCount() == 1 && sound(0).get("sound_id").equals(2393)
            && sound(0).get("loops").equals(1) && sound(0).get("delay").equals(7), "Native SFX values changed");
        tests.add(BindingCache.map("case", "script-sfx-id-loops-delay", "queue", List.of(sound(0)),
            "callbacks", new ArrayList<>(callbackEvents)));
        volumes[0] = 0;
        effect(2725, 1, 2);
        require(soundCount() == 1, "Muted SFX should not enqueue");
        volumes[0] = 127;
        effect(2725, 0, 2);
        require(soundCount() == 1, "Zero-repeat SFX should not enqueue");
        tests.add(BindingCache.map("case", "sfx-mute-and-zero-repeat", "queue_size", soundCount()));
        bi.kv.ag(0);
        effect(2735, 1, 2);
        List<Object> countdown = new ArrayList<>();
        for (int tick = 1; tick <= 4; tick++)
        {
            sourceClient.ib((byte) 0);
            countdown.add(BindingCache.map("client_cycle", tick, "queue_size", soundCount(),
                "entry", soundCount() > 0 ? sound(0) : null));
        }
        require(soundCount() == 0, "Dispatched source sound was not removed on the following cycle");
        Map<?, ?> firstCycle = (Map<?, ?>) countdown.get(0);
        Map<?, ?> thirdCycle = (Map<?, ?>) countdown.get(2);
        require(((Map<?, ?>) firstCycle.get("entry")).get("delay").equals(1)
            && ((Map<?, ?>) thirdCycle.get("entry")).get("delay").equals(-100),
            "Native delay countdown/dispatch sentinel differs");
        tests.add(BindingCache.map("case", "native-client-sfx-tick-delay", "submitted_delay", 2, "observed", countdown));
        al fullEffect = al.af(tz.hq, 2015, 0);
        aj fullSound = fullEffect.ab();
        al trimmedEffect = al.af(tz.hq, 2015, 0);
        int trimCycles = trimmedEffect.am();
        aj trimmedSound = trimmedEffect.ab();
        int trimFrames = trimCycles * 441;
        require(trimCycles == 1 && Arrays.equals(Arrays.copyOfRange(fullSound.af, trimFrames, fullSound.af.length), trimmedSound.af),
            "Original leading-delay trimming changed the waveform instead of moving its queue offset");
        tests.add(BindingCache.map("case", "native-leading-delay-trim-is-exact",
            "sound_id", 2015, "trim_client_cycles", trimCycles, "trim_frames", trimFrames,
            "remaining_pcm_identical", true, "full_frames", fullSound.af.length, "trimmed_frames", trimmedSound.af.length));
        bi.kv.ag(0);
        for (int i = 0; i < 51; i++) effect(2393, 1, i);
        require(soundCount() == 50 && sound(49).get("delay").equals(49), "Native fifty-entry FIFO bound differs");
        tests.add(BindingCache.map("case", "sfx-capacity-fifo", "queued", soundCount(), "last", sound(49)));

        for (int[] order : new int[][]{{154, 33}, {33, 154}, {152, 154}, {154, 152}})
        {
            resetJingles();
            jingle(order[0], 0);
            jingle(order[1], 0);
            require(pending().equals(List.of(order[1])) && client.ka, "Native pending-jingle replacement differs");
            tests.add(BindingCache.map("case", "jingle-request-order", "submitted", order, "pending", pending(),
                "playing_jingle", client.ka));
        }
        for (int argument : new int[]{0, 1, 255, 60000})
        {
            resetJingles();
            jingle(154, argument);
            require(pending().equals(List.of(154)), "The auxiliary jingle argument changed the selector");
            tests.add(BindingCache.map("case", "jingle-auxiliary-argument", "argument", argument, "pending", pending()));
        }
        resetJingles();
        musicVolume(0);
        jingle(154, 0);
        require(pending().isEmpty() && !client.ka, "Muted jingle should not start");
        musicVolume(128);
        jingle(-1, 0);
        require(pending().isEmpty() && !client.ka, "Jingle sentinel should not start");
        jingle(154, 0);
        jingle(-1, 60000);
        require(pending().equals(List.of(154)), "Sentinel must not erase an earlier accepted request");
        tests.add(BindingCache.map("case", "jingle-mute-sentinel", "final_pending", pending()));

        resetJingles();
        nb background = new nb(SourceAudio.OfflineArchive.load(inputs, 6), 76, 0, 128, false);
        background.al = new nu(new ak());
        np.ac.add(background);
        np.ab.add(background);
        jingle(154, 0);
        require(np.ab.isEmpty() && np.ac.size() == 1 && np.ac.get(0) == background,
            "Jingle must clear active streams while retaining the background playlist");
        tests.add(BindingCache.map("case", "jingle-replaces-active-retains-playlist",
            "active_streams", np.ab.size(), "remembered_background_group", background.getArchiveId(), "pending", pending()));

        Files.createDirectories(output.getParent());
        Files.writeString(output, new GsonBuilder().setPrettyPrinting().serializeNulls().create().toJson(
            BindingCache.map("source_runtime", "injected1.12.38", "result", "passed", "cases", tests,
                "interpretation", "Supplied IDs exercise generic native queue rules; they do not prove per-quest server selectors.",
                "account_login", false, "audio_device_opened", false, "presentation_accepted", false)) + "\n");
        System.out.println("Passed " + tests.size() + " native queue/opcode cases; no player session or audio device.");
    }

    public static void main(String[] args)
    {
        try
        {
            if (args.length != 2) throw new IllegalArgumentException("NativeBindingProbe verified-audio-inputs output.json");
            run(Path.of(args[0]), Path.of(args[1]));
        }
        catch (Exception exception)
        {
            exception.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
