import java.nio.file.Files;
import java.nio.file.Path;
import java.lang.reflect.Field;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.runelite.cache.definitions.ScriptDefinition;
import net.runelite.cache.definitions.loaders.DBRowLoader;
import net.runelite.cache.definitions.loaders.EnumLoader;

/** Bounded original script/varbit probes; no game session, persisted account or native patches. */
public final class NativePreferenceControls
{
    static final int[] ROOTS = {250,313,315,318,7109,9232,9234,9238,9240,9244,9246,
        9250,9252,9253,9254,9255,9292,9295,9296,9297,9304,9305,9306,9630};
    static final Map<Integer, ScriptDefinition> scripts = new LinkedHashMap<>();
    static final List<Object> inputs = new ArrayList<>();
    static BindingCache cache;
    static final List<Object> cases = new ArrayList<>();

    static void host() throws Exception
    {
        var configs = PreferenceArchive.open(cache, 2);
        Field slots = client.class.getDeclaredField("ov");
        slots.setAccessible(true);
        var archives = (vp[]) slots.get(null);
        for (int id : new int[]{2,3,7,8,12,13,17,21,22}) archives[id] = PreferenceArchive.open(cache,id);
        wn.hs = archives[12];
        be.in = archives[21];
        pt.ae = configs;
        oy.ae = configs;
        lh.aa = configs;
        bf.hu = configs;
        pa.ae = configs;
        pn.ax = configs;
        zw.ae = configs;
        pr.af = configs;
        ze.ab = configs;
        zu.ae = configs;
        lb.af = new int[6000];
        client.al();
        client.gc(-1);
        is.dk = new dz(0,104,104,25,ex.az);
        client.dr.ad(-1780765646);
        client.dr.as = is.dk;
        client.dr.az.af(is.dk,0L);
        wk.cy = new vv(archives[3],archives[7],archives[8],archives[13],null);
        sx.et = new FixtureVarcs();
        em.fa((byte)0);
    }

    static List<Integer> script(int id, Object... arguments)
    {
        Object[] call = new Object[arguments.length+1];
        call[0] = id;
        System.arraycopy(arguments,0,call,1,arguments.length);
        NativePolicyProbe.instance.runScript(call);
        var stack = new ArrayList<Integer>();
        for (int i=0;i<dy.aq*-324749371;i++) stack.add(bb.ay[i]);
        return stack;
    }

    static Map<String,Object> volumes()
    {
        return SourceAudio.map("current",List.of(lb.af[3796],lb.af[168],lb.af[169],lb.af[872]),
            "remembered",List.of(NativePolicyProbe.instance.getVarbitValue(14817),
                NativePolicyProbe.instance.getVarbitValue(12426),NativePolicyProbe.instance.getVarbitValue(12427),
                NativePolicyProbe.instance.getVarbitValue(12428)), "native_mixer",List.of(
                    mh.gv((byte)11),kf.kw.as(0),kf.kw.ag(0)));
    }

    static Map<String,Object> volumeProof()
    {
        var callback = volumes();
        script(7109); script(2475); script(3643); script(3644);
        return SourceAudio.map("callback",callback,"after_original_option_sync",volumes());
    }

    static void seedVolumes(int[] current, int[] saved)
    {
        int[] varps = {3796,168,169,872}, bits = {14817,12426,12427,12428};
        for (int i=0;i<4;i++)
        {
            lb.af[varps[i]] = current[i];
            NativePolicyProbe.instance.setVarbit(bits[i],saved[i]);
        }
        script(7109);
        script(2475);
        script(3643);
        script(3644);
        bi.kv.ag(0);
    }

    static List<Integer> groups(List<?> songs)
    {
        return songs.stream().map(value -> ((nb)value).getArchiveId()).toList();
    }

    static Map<String,Object> music()
    {
        return SourceAudio.map("remembered",groups(np.ac),"requested",groups(np.ae),
            "jingle_active",client.ka,"transition",List.of(np.ao*-1331669075,np.al*1784906769,
                np.aj*-1350272915,np.ay*396217257));
    }

    static void clearMusic()
    {
        np.ae.clear(); np.ab.clear(); np.aa.clear(); np.ac.clear();
        client.ka = false;
        bi.kv.ag(0);
    }

    static void request(int group)
    {
        rj.bc(new ArrayList<>(List.of(group)),0,60,60,0,(byte)53);
    }

    static int returned(int id, Object... arguments)
    {
        var result = script(id,arguments);
        NativePolicyProbe.require(result.size()==1,"Native script did not return exactly one value: "+id+" "+result);
        return result.get(0);
    }

    static void slots(int slot)
    {
        int get = 9316+(slot-1)*2, set = get+1;
        int add = 9308+(slot-1)*3, remove = add+1, contains = add-1;
        NativePolicyProbe.require(returned(get,1)==-1 && returned(get,100)==-1,"Native empty playlist changed");
        script(set,1,2549);
        script(set,3,2777);
        script(set,100,2938);
        NativePolicyProbe.require(returned(add,2674)==1 && returned(get,2)==2674,"Native first-hole insertion changed");
        NativePolicyProbe.require(returned(remove,2549)==1 && returned(get,1)==-1 &&
            returned(get,2)==2674 && returned(get,3)==2777 && returned(get,100)==2938,
            "Native deletion compacted or overwrote another entry");
        int varp = 5239+(slot-1)*50;
        NativePolicyProbe.require(lb.af[varp]==(203<<16) && lb.af[varp+1]==226 &&
            lb.af[varp+49]==(2700<<16),"Native source track encoding changed");
        NativePolicyProbe.require(returned(contains,2777)==1 && returned(contains,2549)==0,
            "Native playlist membership changed");
        cases.add(SourceAudio.map("case","saved-playlist-"+slot+"-sparse-roundtrip","slot",slot,
            "varp_start",varp,"packed_words",List.of(lb.af[varp],lb.af[varp+1],lb.af[varp+49]),
            "source_rows_at_1_2_3_100",List.of(returned(get,1),returned(get,2),returned(get,3),returned(get,100)),
            "first_hole_reused",2,"removal_compacts",false));
    }

    static void toggle(int channel)
    {
        int setting = new int[]{319,30,31,32}[channel];
        int thumb = 116<<16 | (95+14*channel);
        int track = 116<<16 | (94+14*channel);
        script(9255,setting,thumb,track,0,0,0,0,"","");
    }

    static void execute(Path output) throws Exception
    {
        host();
        for (int channel=0;channel<4;channel++) toggle(channel);
        NativePolicyProbe.require(lb.af[3796]==100 && lb.af[168]==20 && lb.af[169]==45 && lb.af[872]==25,
            "Original first-use restore percentages changed: "+volumes());
        cases.add(SourceAudio.map("case","first-use-zero-memory","observed",volumeProof()));
        seedVolumes(new int[]{0,0,0,0},new int[]{37,21,66,83});
        for (int channel=0;channel<4;channel++) toggle(channel);
        NativePolicyProbe.require(lb.af[3796]==37 && lb.af[168]==21 && lb.af[169]==66 && lb.af[872]==83,
            "Original saved percentages were not restored");
        cases.add(SourceAudio.map("case","restore-saved-vector","observed",volumeProof()));
        seedVolumes(new int[]{37,21,66,83},new int[]{100,20,45,25});
        for (int channel=0;channel<4;channel++) toggle(channel);
        NativePolicyProbe.require(lb.af[3796]==0 && lb.af[168]==0 && lb.af[169]==0 && lb.af[872]==0,
            "Original mute did not zero current settings");
        NativePolicyProbe.require(NativePolicyProbe.instance.getVarbitValue(14817)==37 &&
            NativePolicyProbe.instance.getVarbitValue(12426)==21 &&
            NativePolicyProbe.instance.getVarbitValue(12427)==66 &&
            NativePolicyProbe.instance.getVarbitValue(12428)==83,"Original mute did not remember current settings");
        cases.add(SourceAudio.map("case","mute-positive-vector","observed",volumeProof()));
        fc.hy = PreferenceArchive.open(cache,6);
        cb.hn = PreferenceArchive.open(cache,11);
        for (int mode : new int[]{0,2,1})
        {
            seedVolumes(new int[]{100,100,100,100},new int[]{0,0,0,0});
            clearMusic();
            lb.af[18] = mode;
            request(62);
            var before = music();
            script(9292,0);
            NativePolicyProbe.require(bi.kv.af*2086456713==0,"Non-primary source Skip action was accepted");
            script(9292,1);
            int count = bi.kv.af*2086456713;
            NativePolicyProbe.require(count==(mode==1?1:0) && music().equals(before),
                "Source Skip guard/callback-only boundary changed");
            var click = count==0 ? null : bi.kv.ae[0];
            if (click!=null) NativePolicyProbe.require(click.ax*981301933==2266 &&
                click.as*264987669==1 && click.ag*-798422579==0,"Source Skip click binding changed");
            cases.add(SourceAudio.map("case","skip-mode-"+mode,"mode",mode,"primary_click_queued",count,
                "nonprimary_click_queued",0,"source_sound",click==null?null:2266,"repeat",click==null?null:1,
                "delay_cycles",click==null?null:0,"callback_changes_music",false,"native_music_before_and_after",before));
        }
        clearMusic();
        request(62);
        NativePolicyProbe.opcode(3202,152,0);
        var jingle = groups(np.ae);
        var sameGroup = music();
        request(62);
        NativePolicyProbe.require(music().equals(sameGroup),"Native same-group guard changed the jingle's remembered transition");
        request(76);
        NativePolicyProbe.require(groups(np.ae).equals(jingle) && groups(np.ac).equals(List.of(76)) &&
            np.al*1784906769==60 && np.aj*-1350272915==60,
            "A background request during a jingle did not replace remembered selection/transition");
        var first = music();
        request(327);
        NativePolicyProbe.require(groups(np.ae).equals(jingle) && groups(np.ac).equals(List.of(327)),
            "Native last background request during jingle did not win");
        cases.add(SourceAudio.map("case","background-request-during-jingle","same_group_noop",sameGroup,"first",first,"last",music(),
            "jingle_restarted_or_replaced",false));
        clearMusic();
        request(62);
        NativePolicyProbe.opcode(3203,0);
        var beforeMuted = music();
        script(9292,1);
        request(76);
        NativePolicyProbe.require(music().equals(beforeMuted),"Native zero-mixer request changed remembered music");
        cases.add(SourceAudio.map("case","muted-shuffle-request","native_mixer",mh.gv((byte)11),
            "native_music_before_and_after",beforeMuted,"click_queued",bi.kv.af*2086456713,
            "next_selection_consumed",false));
        for (int slot=1;slot<=3;slot++) slots(slot);
        lb.af[18] = 0;
        lb.af[3883] = 2777;
        NativePolicyProbe.instance.setVarbit(19731,0);
        NativePolicyProbe.instance.setVarbit(19736,0);
        var rootWidget = wk.cy.az(239<<16,(byte)82);
        Object[] load = rootWidget.getOnLoadListener().clone();
        load[1] = 239<<16;
        NativePolicyProbe.instance.runScript(load);
        Object[] dropdown = wk.cy.az(239<<16|18,(byte)82).getOnOpListener();
        NativePolicyProbe.instance.runScript(dropdown);
        Object[] select = wk.cy.az(239<<16|21,(byte)82).getChild(3).getOnOpListener();
        NativePolicyProbe.require(select[0].equals(9297) && select[3].equals(1),
            "Original Playlist 1 callback identity changed");
        NativePolicyProbe.instance.runScript(select);
        NativePolicyProbe.require(lb.af[18]==1 && NativePolicyProbe.instance.getVarbitValue(19731)==1,
            "Original playlist selection did not move Area to Shuffle");
        cases.add(SourceAudio.map("case","actual-numbered-menu-selection","before_mode",0,"before_slot",0,
            "keep_playing_varbit19736",0,"selected_slot",1,"after_mode",lb.af[18],
            "after_slot",NativePolicyProbe.instance.getVarbitValue(19731),
            "source_generated_callback",select,"native_actions",wk.cy.az(239<<16|17,(byte)82).getActions()));
        NativePolicyProbe.require(cases.size()==12,"This task permits exactly this bounded twelve-state probe");
        Files.writeString(output.resolve("native.json"),SourceAudio.JSON.toJson(
            SourceAudio.map("result","passed","controlled_native_states",cases.size(),"cases",cases,
                "cache_groups",cache.groups)));
    }

    static void inspect(Path output) throws Exception
    {
        Files.createDirectories(output.resolve("scripts"));
        var queue = new ArrayDeque<Integer>();
        for (int id : ROOTS) queue.add(id);
        while (!queue.isEmpty())
        {
            int id = queue.removeFirst();
            if (scripts.containsKey(id)) continue;
            NativePolicyProbe.require(scripts.size() < 160, "Focused script closure exceeded its bound");
            byte[] bytes = cache.group(12, id).findFile(0).getContents();
            var definition = BindingExtract.nativeScript(id, bytes);
            scripts.put(id, definition);
            inputs.add(SourceAudio.map("index",12,"group",id,"file",0,"sha256",SourceAudio.hash(bytes)));
            Files.writeString(output.resolve("scripts/"+id+".json"), SourceAudio.JSON.toJson(
                SourceAudio.map("sha256",SourceAudio.hash(bytes),"definition",definition)));
            int[] ops = definition.getInstructions(), values = definition.getIntOperands();
            for (int pc = 0; pc < ops.length; pc++) if (ops[pc] == 40) queue.add(values[pc]);
        }
        var bits = new ArrayList<Object>();
        var ids = new ArrayList<Integer>(List.of(14817,12426,12427,12428,19731,4137,12233,
            19734,19735,19736,19737,20038,20039,20040,20041,20042,20043,20044,20045));
        for (int id = 19738; id <= 20037; id++) ids.add(id);
        for (int id : ids)
        {
            byte[] bytes = cache.group(2,14).findFile(id).getContents();
            pt value = new pt(new xy(bytes));
            bits.add(SourceAudio.map("id",id,"varp",value.getIndex(),"lsb",value.getLeastSignificantBit(),
                "msb",value.getMostSignificantBit(),"sha256",SourceAudio.hash(bytes)));
        }
        Files.writeString(output.resolve("definitions.json"), SourceAudio.JSON.toJson(
            SourceAudio.map("scripts",inputs,"varbits",bits)));
        var rows = new ArrayList<Object>();
        for (int id : new int[]{2549,2583,2674,2721,2777,2938,3012,3237})
        {
            byte[] raw = cache.group(2,38).findFile(id).getContents();
            var row = new DBRowLoader().load(id,raw);
            Object[][] values = row.getColumnValues();
            rows.add(SourceAudio.map("row",id,"group",values[4][0],"unlock_fields",values[5],
                "stored_track_id",(int)values[5][0]*100+(int)values[5][1],"sha256",SourceAudio.hash(raw)));
        }
        byte[] menu = cache.group(2,8).findFile(2772).getContents();
        var areaModes = new ArrayList<Object>();
        for (var file : cache.group(2,8).getFiles())
        {
            String raw = new String(file.getContents(),java.nio.charset.StandardCharsets.ISO_8859_1);
            if (!raw.contains("Modern") || !raw.contains("Classic")) continue;
            var definition = new EnumLoader().load(file.getFileId(),file.getContents());
            NativePolicyProbe.require(definition!=null,"Selected area-mode enum failed native-format decoding");
            if (definition.getStringVals()==null) continue;
            var labels = java.util.Arrays.asList(definition.getStringVals());
            if (labels.contains("Modern") && labels.contains("Classic"))
                areaModes.add(SourceAudio.map("definition",definition,"sha256",SourceAudio.hash(file.getContents())));
        }
        Files.writeString(output.resolve("music-identities.json"),SourceAudio.JSON.toJson(
            SourceAudio.map("tracks",rows,"playlist_menu",new EnumLoader().load(2772,menu),
                "menu_sha256",SourceAudio.hash(menu),"area_mode_enums",areaModes)));
    }

    public static void main(String[] args)
    {
        try (BindingCache source = new BindingCache(Path.of(args[0])))
        {
            NativePolicyProbe.initialize();
            cache = source;
            inspect(Path.of(args[1]));
            if (args.length > 2 && args[2].equals("run")) execute(Path.of(args[1]));
            System.out.println(SourceAudio.JSON.toJson(SourceAudio.map(
                "bounded_script_closure",scripts.size(),"controlled_native_states",cases.size())));
        }
        catch (Exception failure)
        {
            failure.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
