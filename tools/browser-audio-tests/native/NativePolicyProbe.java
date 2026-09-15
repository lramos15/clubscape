import com.google.gson.GsonBuilder;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.lang.reflect.Proxy;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Locale;
import java.util.LinkedHashSet;
import java.util.LinkedHashMap;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import net.runelite.cache.definitions.loaders.GameValLoader;
import net.runelite.cache.definitions.loaders.DBRowLoader;
import net.runelite.cache.definitions.loaders.DBTableLoader;
import net.runelite.api.hooks.Callbacks;

/** Executes the unchanged locked native classes; only external IO/callback boundaries are isolated. */
public final class NativePolicyProbe
{
    static final List<Object> cases = new ArrayList<>();
    static client instance;

    static void require(boolean condition, String message)
    {
        if (!condition) throw new IllegalStateException(message);
    }

    static int encode(int value, int decoder)
    {
        return value * BigInteger.valueOf(Integer.toUnsignedLong(decoder))
            .modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
    }

    static Object allocate(Class<?> type) throws Exception
    {
        Class<?> unsafe = Class.forName("sun.misc.Unsafe");
        Field field = unsafe.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return unsafe.getMethod("allocateInstance", Class.class).invoke(field.get(null), type);
    }

    static void initialize() throws Exception
    {
        instance = (client) allocate(client.class);
        instance.km = Thread.currentThread();
        Callbacks callbacks = (Callbacks) Proxy.newProxyInstance(Callbacks.class.getClassLoader(),
            new Class<?>[]{Callbacks.class}, (proxy, method, args) ->
            {
                if (method.getReturnType() == boolean.class) return false;
                if (method.getReturnType() == int.class) return 0;
                if (method.getReturnType() == long.class) return 0L;
                return null;
            });
        for (Field field : client.class.getDeclaredFields())
        {
            field.setAccessible(true);
            if (field.getType() == Callbacks.class)
                field.set(Modifier.isStatic(field.getModifiers()) ? null : instance, callbacks);
            if (field.getName().equals("wg") && field.getType() == java.util.concurrent.ScheduledExecutorService.class)
                field.set(instance, new FixturePreferenceWrites());
        }
        oe.cz = instance;
        aal.ae = encode(240, 2098754687);
        ab.kn = new cy();
        kf.kw = new dm();
        bi.kv = new sm(kf.kw);
        qo.ki = new ao();
        lg.al = 22050 * (int) 3883844509L;
        kg.aj = true;
    }

    static int opcode(int code, int... values)
    {
        for (int i = 0; i < values.length; i++) bb.ay[i] = values[i];
        dy.aq = encode(values.length, -324749371);
        int result = ee.bt(code, null, false, (byte) 0);
        require(result == 1, "Unhandled native opcode " + code);
        int count = dy.aq * -324749371;
        return count == 1 ? bb.ay[0] : Integer.MIN_VALUE;
    }

    static Map<String, Object> levels()
    {
        return SourceAudio.map(
            "music_opcode_get", opcode(3204), "effects_opcode_get", opcode(3206), "area_opcode_get", opcode(3208),
            "music_preference", ab.kn.ai((byte) 0),
            "effects_preference", cy.zg(ab.kn, (byte) 18), "area_preference", cy.vb(ab.kn, 0),
            "master_scalar", ab.kn.bk(0),
            "music_mixer", mh.gv((byte) 11), "effects_mixer", kf.kw.as(0), "area_mixer", kf.kw.ag(0),
            "legacy_effects_preference", ab.kn.getSoundEffectVolume(),
            "legacy_area_preference", ab.kn.getAreaSoundEffectVolume(),
            "legacy_8bit_enabled", cy.je(ab.kn, 0));
    }

    static void preferences(Path inputs) throws Exception
    {
        cases.add(SourceAudio.map("case", "constructor-before-native-mixer-refresh", "observed", levels()));
        em.fa((byte) 0);
        require(mh.gv((byte) 11) == 255 && kf.kw.as(0) == 127 && kf.kw.ag(0) == 127 &&
            ab.kn.ai((byte) 0) == 127 && ab.kn.bk(0) == 1.0f,
            "Fresh native mixer/preferences defaults changed");
        cases.add(SourceAudio.map("case", "fresh-native-preferences-after-em-fa", "observed", levels()));
        List<Object> settings = new ArrayList<>();
        for (int percent : new int[]{-1, 0, 1, 5, 10, 20, 25, 50, 75, 80, 100, 101, 255})
        {
            opcode(3203, percent);
            opcode(3205, percent);
            opcode(3207, percent);
            settings.add(SourceAudio.map("input", percent, "observed", levels()));
        }
        cases.add(SourceAudio.map("case", "native-slider-setter-mixer", "observed", settings));
        cases.add(SourceAudio.map("case", "native-nonlinear-lookup-tables",
            "effect_and_area", dm.ab, "music", dm.ae));
        List<Object> gains = new ArrayList<>();
        var effects = SourceAudio.OfflineArchive.load(inputs, 4);
        for (int volume : new int[]{0, 1, 22, 63, 127, 128})
        {
            aj original = al.af(effects, 2735, 0).ab();
            var device = new SourceAudio.OriginalDevice();
            am stream = am.ae(original, 100, volume);
            stream.bw(0);
            device.device.ae(stream, (byte) 0);
            int[] block = new int[1024];
            for (int frame = 0; frame < original.af.length; frame += 512)
            {
                device.device.aa(block, 512);
                device.write(block, Math.min(512, original.af.length - frame));
            }
            byte[] pcm = device.pcm.toByteArray();
            ByteBuffer rendered = ByteBuffer.wrap(pcm).order(ByteOrder.LITTLE_ENDIAN);
            int maximumDifference = 0;
            for (int i = 0; i < original.af.length; i++)
            {
                int expected = original.af[i] * volume >> 8;
                maximumDifference = Math.max(maximumDifference, Math.abs(rendered.getShort() - expected));
                maximumDifference = Math.max(maximumDifference, Math.abs(rendered.getShort() - expected));
            }
            require(maximumDifference == 0, "The original SFX/device gain equation changed");
            gains.add(SourceAudio.map("source_id", 2735, "native_volume", volume,
                "frames", original.af.length, "unattenuated_source_divisor", 256,
                "all_stereo_samples_max_integer_error", maximumDifference,
                "source_pcm", SourceAudio.rawMetadata(original),
                "native_device_pcm_sha256", SourceAudio.hash(pcm),
                "native_device_clipped_samples", device.clipped, "pre_device_peak_s24", device.peak));
        }
        cases.add(SourceAudio.map("case", "original-sfx-player-mixer-device-scaling", "observed", gains));
    }

    static void catalog(Path path) throws Exception
    {
        try (BindingCache cache = new BindingCache(path))
        {
            var loader = new GameValLoader();
            List<Object> groupTypes = new ArrayList<>();
            List<Object> labels = new ArrayList<>();
            for (Field field : GameValLoader.class.getFields())
                if (field.getType() == int.class && Modifier.isStatic(field.getModifiers()))
                    groupTypes.add(SourceAudio.map("name", field.getName(), "group", field.get(null)));
            for (var archive : cache.index(24).getArchives())
            {
                int group = archive.getArchiveId();
                if (group != GameValLoader.VARPS && group != GameValLoader.VARBITS &&
                    group != GameValLoader.VARCS && group != GameValLoader.DBTABLES &&
                    group != GameValLoader.DBROWS && group != GameValLoader.INTERFACES) continue;
                for (var file : cache.group(24, group).getFiles())
                {
                    var value = loader.load(group, file.getFileId(), file.getContents());
                    String name = value.getName() == null ? "" : value.getName().toLowerCase(Locale.ROOT);
                    if ((name.contains("music") || name.contains("audio") || name.contains("volume") || name.contains("sound")) &&
                        !name.matches("music_playlist_[123]_(track_)?[0-9]+") &&
                        !name.matches("musicmulti_[0-9]+") &&
                        group != GameValLoader.DBROWS && group != GameValLoader.INTERFACES)
                        labels.add(SourceAudio.map("group", group, "id", file.getFileId(),
                            "source_sha256", SourceAudio.hash(file.getContents()), "definition", value));
                }

            }
            cases.add(SourceAudio.map("case", "original-audio-gameval-labels", "group_types", groupTypes, "labels", labels));
            var music = cache.index(6);
            List<Object> names = new ArrayList<>();
            for (String name : new String[]{"scape main", "newbie melody", "scape cave", "harmony",
                    "autumn voyage", "book of spells", "dream", "flute salad", "yesteryear"})
            {
                int hash = 0;
                for (char c : name.toCharArray()) hash = 31 * hash + c;
                for (var archive : music.getArchives())
                    if (archive.getNameHash() == hash)
                        names.add(SourceAudio.map("name", name, "group", archive.getArchiveId(),
                            "name_hash", hash, "revision", archive.getRevision(), "crc", archive.getCrc()));
            }
            cases.add(SourceAudio.map("case", "native-music-archive-name-identities", "matches", names));
            var tables = cache.group(2, 39);
            var table = new DBTableLoader().load(44, tables.findFile(44).getContents());
            var table128 = new DBTableLoader().load(128, tables.findFile(128).getContents());
            var rows = cache.group(2, 38);
            var rowLoader = new DBRowLoader();
            List<Object> selected = new ArrayList<>();
            List<Object> areaGroups = new ArrayList<>();
            var selectedAreas = new LinkedHashSet<Object>();
            List<net.runelite.cache.definitions.DBRowDefinition> musicRows = new ArrayList<>();
            for (var file : rows.getFiles())
            {
                var row = rowLoader.load(file.getFileId(), file.getContents());
                if (row.getTableId() == 44)
                {
                    musicRows.add(row);
                    Object[][] values = row.getColumnValues();
                    if (values[4] != null && List.of(0, 2, 62, 76, 144).contains(values[4][0]))
                    {
                        selected.add(SourceAudio.map("source_sha256", SourceAudio.hash(file.getContents()), "definition", row));
                        if (values[7] != null) for (Object area : values[7]) selectedAreas.add(area);
                    }
                }
                else if (row.getTableId() == 128)
                    areaGroups.add(SourceAudio.map("source_sha256", SourceAudio.hash(file.getContents()), "definition", row));
            }
            List<Object> sameArea = new ArrayList<>();
            for (var row : musicRows)
            {
                Object[][] values = row.getColumnValues();
                if (values[7] == null) continue;
                for (Object area : values[7])
                    if (selectedAreas.contains(area))
                    {
                        sameArea.add(SourceAudio.map("source_sha256", SourceAudio.hash(rows.findFile(row.getId()).getContents()), "definition", row));
                        break;
                    }
            }
            cases.add(SourceAudio.map("case", "original-music-table44-and-area-group128",
                "table44", table, "table128", table128, "selected_rows", selected,
                "selected_areas", selectedAreas, "same_area_rows", sameArea, "area_groups", areaGroups));
            cases.add(SourceAudio.map("case", "readonly-native-cache-inputs", "groups", cache.groups));
        }
    }

    static byte[] mix(ah stream, int frames) throws Exception
    {
        var device = new SourceAudio.OriginalDevice();
        device.device.ae(stream, (byte) 0);
        int[] block = new int[1024];
        for (int frame = 0; frame < frames; frame += 512)
        {
            device.device.aa(block, 512);
            device.write(block, Math.min(512, frames - frame));
        }
        return device.pcm.toByteArray();
    }

    static void position(Path inputs, Path cachePath) throws Exception
    {
        tz.hq = SourceAudio.OfflineArchive.load(inputs, 4);
        em.fa((byte) 0);
        client.dr.ad(-1780765646);
        dz main = (dz) allocate(dz.class);
        main.ad = new yn(16);
        main.af = 0;
        client.dr.as = main;
        client.dr.az.af(main, 0L);
        is.dk = main;
        client.da = 0;
        List<Object> queueCases = new ArrayList<>();
        aj original = al.af(tz.hq, 2735, 0).ab();
        for (int retain : new int[]{0, 1, 3, 7})
            for (int[] point : new int[][]{{1344,1344},{1408,1344},{1472,1344},{1536,1472},{1727,1344},{1856,1344},{1984,1344}})
            {
                qo.ki = new ao();
                bi.kv.ag(0);
                client.np = encode(point[0], 2106329293);
                client.nq = encode(point[1], -2126074583);
                sb request = new sb();
                request.ax = encode(2735, 981301933);
                request.as = encode(1, 264987669);
                request.ag = encode(0, -798422579);
                request.ab = encode((10 << 16) | (10 << 8) | 5, 1410178907);
                request.af = encode(retain, 174419521);
                request.ae = 0;
                request.ac = false;
                bi.kv.ae[0] = request;
                bi.kv.af = encode(1, 2086456713);
                instance.ib((byte) 0);
                byte[] actual = mix(qo.ki, original.af.length);
                int distance = Math.max(0, Math.abs(1344 - point[0]) + Math.abs(1344 - point[1]) - 128);
                int range = 5 * 128, retained = Math.max(0, ((retain & 31) - 1) * 128);
                float factor = retained >= range ? 1 : Math.min(1, Math.max(0, (float)(range - distance) / (range - retained)));
                int expectedVolume = distance >= range ? 0 : (int) Math.ceil(factor * 127);
                ByteBuffer pcm = ByteBuffer.wrap(actual).order(ByteOrder.LITTLE_ENDIAN);
                int maxError = 0;
                for (short sample : original.af)
                {
                    int expected = sample * expectedVolume >> 8;
                    maxError = Math.max(maxError, Math.abs(pcm.getShort() - expected));
                    maxError = Math.max(maxError, Math.abs(pcm.getShort() - expected));
                }
                require(maxError == 0, "Native source-cycle position/mixer formula mismatch: " + maxError);
                queueCases.add(SourceAudio.map("listener", point, "source_tile", List.of(10,10),
                    "range_field", 5, "retain_field", retain, "distance_source_units", distance,
                    "retained_source_units", retained, "expected_native_volume", expectedVolume,
                    "all_stereo_samples_integer_error", maxError, "native_pcm_sha256", SourceAudio.hash(actual)));
            }
        cases.add(SourceAudio.map("case", "actual-native-client-ib-position-and-pcm", "observed", queueCases));
        List<Object> distances = new ArrayList<>();
        for (int[] p : new int[][]{{1280,1280},{1344,1344},{1536,1536},{1600,1600},{1664,1664},{1024,1024},{1700,1400}})
        {
            int nativeValue = cl.av(p[0], p[1], 1280, 1280, 1536, 1536, -2135106240);
            int expected = Math.max(0, Math.max(0, Math.max(1280-p[0],p[0]-1536)) +
                Math.max(0, Math.max(1280-p[1],p[1]-1536)) - 64);
            require(nativeValue == expected, "Native ambient rectangle distance mismatch");
            distances.add(SourceAudio.map("listener", p, "bounds", List.of(1280,1280,1536,1536),
                "native_distance", nativeValue, "independent_expected", expected));
        }
        cases.add(SourceAudio.map("case", "native-ambient-rectangle-manhattan-minus64", "observed", distances));
        sp a = (sp) allocate(sp.class), b = (sp) allocate(sp.class);
        var visibility = Class.forName("rq").getDeclaredMethod("az", sp.class, sp.class, boolean.class, byte.class);
        visibility.setAccessible(true);
        List<Object> ownership = new ArrayList<>();
        for (int listener = 0; listener < 3; listener++)
            for (int emitter = 0; emitter < 3; emitter++)
                for (boolean cross : new boolean[]{false,true})
                {
                    sp l = listener == 0 ? null : listener == 1 ? a : b;
                    sp e = emitter == 0 ? null : emitter == 1 ? a : b;
                    boolean actual = (boolean) visibility.invoke(null, l, e, cross, (byte) -1);
                    boolean expected = l == e || e == null || (l != null && cross);
                    require(actual == expected, "Native world-entity ownership visibility mismatch");
                    ownership.add(SourceAudio.map("listener_owner", listener, "emitter_owner", emitter,
                        "cross_owner_flag", cross, "native_audible", actual));
                }
        cases.add(SourceAudio.map("case", "native-rq-world-entity-visibility", "observed", ownership));
        try (BindingCache cache = new BindingCache(cachePath))
        {
            var configs = NativeArchive.open(cache, 2);
            ak.cq = configs;
            pt.ae = configs;
            lb.af = new int[65536];
            om range = (om) instance.getObjectDefinition(114);
            List<Object> ambient = new ArrayList<>();
            for (int x : new int[]{1344,1472,1536,1664,1792,1856,1920})
            {
                cr emitter = new cr(0, 1280, 1280, 1408, 1408, range);
                emitter.az(0);
                emitter.ak(x,1344,1280,1280,1408,1408,1,true,-317896765);
                int d = Math.max(0, x - 1408 - 64);
                int expected = d > 384 ? 0 : (int)Math.ceil(Math.max(0,Math.min(1,(384-d)/384.0))*127);
                int actual = emitter.al == null ? 0 : emitter.al.ab(0);
                require(actual == expected, "Native original cooking-range volume mismatch");
                ambient.add(SourceAudio.map("listener_x",x, "source_id",emitter.getSoundEffectId(),
                    "plane",emitter.getPlane(),"native_radius",emitter.ax * -1766162897,
                    "native_retain",emitter.ac * -875852501,
                    "continuous_stream", emitter.al != null,
                    "initial_stream_volume",actual,"independent_expected",expected));
                emitter.ah(1940029505);
            }
            cases.add(SourceAudio.map("case", "actual-original-cooking-range-emitter", "observed", ambient));
            List<Object> planes = new ArrayList<>();
            for (int visibilityId : new int[]{0,1,2})
            {
                kc visibilityKind = null;
                for (kc v : kc.as()) if (v.az((byte) 0) == visibilityId) visibilityKind=v;
                require(visibilityKind != null,"Missing native visibility selector");
                range.df.ag = visibilityKind;
                for (int plane=0;plane<4;plane++)
                {
                    cr emitter = new cr(0,1280,1280,1408,1408,range);
                    boolean visible = main.aj(null,plane,emitter,-12900460);
                    require(visible == (plane==0),"Native main-world plane visibility changed");
                    planes.add(SourceAudio.map("source_plane",0,"listener_plane",plane,
                        "visibility_id",visibilityId,"native_visible",visible));
                }
            }
            cases.add(SourceAudio.map("case","actual-dz-ambient-plane-visibility","observed",planes));
            List<Object> fades = new ArrayList<>();
            for (int[] control : new int[][]{{127,0,127,300},{127,63,127,300},{63,127,127,300},{0,127,127,300},{127,0,127,150}})
            {
                wc stream = new wc(original,control[0],-1);
                stream.af(control[0],control[2],control[3],range.df.ab.ac(0),0);
                stream.af(control[1],control[2],control[3],range.df.ab.ac(0),0);
                int duration = stream.ag * -665718745;
                int difference = control[0]-control[1];
                int expectedDuration = difference < control[2]
                    ? (int)(control[3]*((float)difference/control[2])) : control[3];
                require(duration == expectedDuration,"Native signed ambient duration scaling changed: "
                    +java.util.Arrays.toString(control)+" observed="+duration+" expected="+expectedDuration);
                List<Integer> samples = new ArrayList<>();
                for (int time : new int[]{0,1,20,75,150,225,300,400})
                    samples.add(stream.ax(control[0],time));
                fades.add(SourceAudio.map("start",control[0],"target",control[1],"base",control[2],
                    "configured_ms",control[3],"native_duration_ms",duration,
                    "elapsed_ms",List.of(0,1,20,75,150,225,300,400),"native_volumes",samples));
            }
            cases.add(SourceAudio.map("case","native-wc-signed-fade-and-volume-rounding","observed",fades));
        }
    }

    static nu midi(Path inputs, int group) throws Exception
    {
        nu synth = new nu(new ak());
        synth.ap(9,128,(short)-27396);
        synth.aj(new no(new xy(Files.readAllBytes(inputs.resolve("raw/6/"+group+"/0.bin")))),false,(byte)2);
        require(nu.sc(synth,(byte)10),"Original MIDI reader is not active");
        return synth;
    }

    static void music(Path inputs) throws Exception
    {
        fc.hy = SourceAudio.OfflineArchive.load(inputs,6);
        List<Object> fadeCases = new ArrayList<>();
        for (int volume : new int[]{22,128,255})
            for (int length : new int[]{0,1,30,60})
                for (boolean fadeIn : new boolean[]{false,true})
                {
                    np.ab.clear();
                    nb song = new nb(fc.hy,62,0,volume,false);
                    song.al=midi(inputs,62);
                    song.ag=fadeIn?0:volume;
                    song.al.az((int)song.ag,0);
                    np.ab.add(song);
                    wt task=fadeIn?new wp(null,0,false,length):new wo(null,0,false,length);
                    var values=new ArrayList<Integer>();values.add((int)song.ag);
                    float expected=song.ag;float step=length==0?volume:(float)volume/length;
                    int processingCalls=0;
                    while(!task.az((byte)2))
                    {
                        processingCalls++;
                        expected=fadeIn?Math.min(volume,expected+(step==0?volume:step))
                            :Math.max(0,expected-(step==0?volume:step));
                        require(song.ag==expected && song.al.af((byte)0)==(int)expected,"Original music fade mismatch");
                        values.add(song.al.af((byte)0));
                        require(processingCalls<length+3,"Unexpected unbounded music fade");
                    }
                    fadeCases.add(SourceAudio.map("volume",volume,"fade_cycles",length,"direction",fadeIn?"in":"out",
                        "native_volumes",values,"completion_processing_call",processingCalls+1));
                }
        cases.add(SourceAudio.map("case","original-wo-wp-music-fade-steps","observed",fadeCases));
        List<Object> delays=new ArrayList<>();
        for(int length:new int[]{0,1,30,60})
        {
            wu delay=new wu(null,length);int calls=0;
            while(!delay.az((byte)2)){calls++;require(calls<=length,"Native delay overrun");}
            require(calls==length,"Native delay changed");
            delays.add(SourceAudio.map("delay_cycles",length,"completes_on_call",calls+1));
        }
        cases.add(SourceAudio.map("case","original-wu-delay-countdown","observed",delays));
        np.ae.clear();np.aa.clear();np.ab.clear();np.ac.clear();
        em.fa((byte)0);
        var list=new ArrayList<Integer>();list.add(62);
        rj.bc(list,0,60,60,0,(byte)53);
        var before=SourceAudio.map("out_delay",np.ao * -1331669075,"out_duration",np.al * 1784906769,
            "in_delay",np.aj * -1350272915,"in_duration",np.ay * 396217257);
        cb.hn=SourceAudio.OfflineArchive.load(inputs,11);
        opcode(3202,152,0);
        var after=SourceAudio.map("out_delay",np.ao * -1331669075,"out_duration",np.al * 1784906769,
            "in_delay",np.aj * -1350272915,"in_duration",np.ay * 396217257);
        cases.add(SourceAudio.map("case","source-music-parameters-across-jingle","before",before,"during",after,
            "remembered_groups",np.ac.stream().map(v->((nb)v).getArchiveId()).toList()));
    }

    public static void main(String[] args)
    {
        try
        {
            require(args.length == 4, "NativePolicyProbe mode inputs cache output");
            initialize();
            switch (args[0])
            {
                case "preferences": preferences(Path.of(args[1])); break;
                case "catalog": catalog(Path.of(args[2])); break;
                case "position": position(Path.of(args[1]), Path.of(args[2])); break;
                case "music": music(Path.of(args[1])); break;
                case "objects": NativeObjects.run(Path.of(args[2])); break;
                case "pcm": NativeMusicPcm.run(Path.of(args[1]),Path.of(args[3]).getParent().resolve("pcm")); break;
                case "scripts":
                    try (BindingCache cache = new BindingCache(Path.of(args[2])))
                    {
                        NativeScriptPolicy.scan(cache, Path.of(args[3]).getParent().resolve("scripts"));
                    }
                    break;
                default: throw new IllegalArgumentException("Unimplemented probe mode " + args[0]);
            }
            Files.writeString(Path.of(args[3]), new GsonBuilder().setPrettyPrinting().serializeNulls().create().toJson(
                SourceAudio.map("schema_version", 1, "runtime", "unmodified injected-client-1.12.38",
                    "game_build", 240, "cache_id", 2695, "result", "passed", "cases", cases,
                    "external_account", false, "personal_preferences_read", false, "hardware_audio_opened", false,
                    "source_pack_modified", false, "acceptance", false)) + "\n");
        }
        catch (Throwable failure)
        {
            failure.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
