import com.google.gson.JsonObject;
import java.awt.Canvas;
import java.awt.event.MouseWheelEvent;
import java.lang.reflect.Field;
import java.lang.reflect.Proxy;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.runelite.api.Client;
import net.runelite.api.hooks.Callbacks;

/** Drives original normal-camera methods in a controlled, offline native world. */
public final class CameraProbe
{
    static final String[] INTS = {"dn","ek","ng","np","nq","md","nr","do","jm","jh","jp","jw","kx"};
    static final int[] DECODERS = {-496203881,1778071523,1741031323,2106329293,-2126074583,-719672733,
        1423746095,-184240077,-1414186039,-1891703075,-958869211,1832988085,1762426687};
    final OriginalCapture capture;
    final LayerCapture host;
    final List<Object> rows = new ArrayList<>();
    final Map<String,Object> initial;
    final Map<Integer,Object> scriptEvidence=new java.util.TreeMap<>();
    int step;
    int width=1920,height=1080;
    static final String[] SCENARIOS={"initial","keyboard","clamps","mouse","follow","follow-threshold",
        "frame-rates","resize","terrain","wheel","orbit","region-shift","zoom-separation","script-init-scan"};

    static Object get(Object target,Class<?> type,String name,Class<?> fieldType) throws Exception
    {
        return LayerCapture.field(target,type,name,fieldType);
    }

    static void put(Object target,Class<?> type,String name,Class<?> fieldType,Object value) throws Exception
    {
        LayerCapture.set(target,type,name,fieldType,value);
    }

    static int logical(Class<?> type,String name,int decoder) throws Exception
    {
        return (int)get(null,type,name,int.class)*decoder;
    }

    static void integer(Class<?> type,String name,int decoder,int value) throws Exception
    {
        WorldCapture.logicalInt(null,type,name,decoder,value);
    }

    static Map<String,Object> cameraState(Client client) throws Exception
    {
        Map<String,Object> nativeState = new LinkedHashMap<>();
        for(int i=0;i<INTS.length;i++)
        {
            int raw=(int)get(null,client.class,INTS[i],int.class);
            nativeState.put(INTS[i],OriginalCapture.map("raw",raw,"decoded",DECODERS[i]==0?null:raw*DECODERS[i]));
        }
        return OriginalCapture.map(
            "source_cycle",client.getGameCycle(),"camera_mode",client.getCameraMode(),
            "locked",get(null,client.class,"kb",boolean.class),
            "target_pitch",client.getCameraPitchTarget(),"target_yaw",client.getCameraYawTarget(),
            "camera_x_height_y",new int[]{client.getCameraX(),client.getCameraZ(),client.getCameraY()},
            "camera_pitch",client.getCameraPitch(),"camera_yaw",client.getCameraYaw(),
            "focal_x_y",new int[]{logical(jy.class,"mg",318828117),logical(pf.class,"mk",68756747)},
            "focal_height",logical(bk.class,"ma",-961764289),
            "focal_float_x_height_y",new float[]{client.getCameraFocalPointX(),client.getCameraFocalPointY(),client.getCameraFocalPointZ()},
            "camera_float_x_height_y",new float[]{client.getCameraFpX(),client.getCameraFpZ(),client.getCameraFpY()},
            "target_float_pitch_yaw",new float[]{(float)get(null,client.class,"vt",float.class),(float)get(null,client.class,"bs",float.class)},
            "native_state",nativeState,
            "mouse_camera_enabled",get(null,on.class,"hd",boolean.class),
            "runelite_camera_speed",get(null,client.class,"ct",float.class),
            "runelite_mouse_button_mask",get(null,client.class,"yz",int.class),
            "runelite_yaw_velocity",get(null,client.class,"tt",int.class),
            "runelite_pitch_velocity",get(null,client.class,"cn",int.class),
            "native_pitch_relaxer",get(null,client.class,"dv",boolean.class),
            "viewport_zoom",logical(client.class,"fk",1129651895),
            "viewport",new int[]{client.getViewportWidth(),client.getViewportHeight()},
            "viewport_distance_scale_shorts",new short[]{(short)get(null,client.class,"fi",short.class),(short)get(null,client.class,"fb",short.class)},
            "viewport_projection_shorts",new short[]{(short)get(null,client.class,"fy",short.class),(short)get(null,client.class,"fg",short.class)});
    }

    CameraProbe(OriginalCache cache,Path scratch,Map<String,Object> initial) throws Exception
    {
        this.initial=initial;
        capture=new OriginalCapture(cache,scratch);
        JsonObject state=new JsonObject();
        state.addProperty("id","normal-camera-probe");
        state.addProperty("camera","lumbridge-castle-plaza");
        state.addProperty("layer","baseline");
        state.addProperty("hide_roofs",false);
        state.addProperty("plane",0);
        state.add("player_tile",OriginalCapture.JSON.toJsonTree(new int[]{3222,3218}));
        host=new LayerCapture(capture,state);
        host.world();
        host.hud();
        capture.game.setCameraMode(0);
        WorldCapture.staticField(client.class,"kb",boolean.class,false);
        capture.game.setCameraPitchTarget(1024);
        capture.game.setCameraYawTarget(0);
        WorldCapture.staticField(client.class,"ct",float.class,1.0f);
        WorldCapture.staticField(client.class,"yz",int.class,0);
        WorldCapture.staticField(client.class,"dv",boolean.class,false);
        integer(client.class,"ek",1778071523,0);
        WorldCapture.staticField(le.class,"ea",ku.class,ku.az);
        integer(jy.class,"mg",318828117,0);
        integer(pf.class,"mk",68756747,0);
        integer(bk.class,"ma",-961764289,0);
        integer(ki.class,"jl",-325062789,0);
        integer(nl.class,"jr",1615527037,0);
        integer(ai.class,"jo",1343311673,0);
        WorldCapture.staticField(client.class,"jj",up.class,new up(0));
        WorldCapture.staticField(client.class,"jt",up.class,new up(0));
        WorldCapture.staticField(client.class,"ln",float.class,0.0f);
        WorldCapture.staticField(client.class,"bn",float.class,0.0f);
        WorldCapture.staticField(client.class,"au",float.class,0.0f);
        integer(client.class,"jm",-1414186039,0);
        integer(client.class,"jh",-1891703075,0);
        WorldCapture.staticField(client.class,"tt",int.class,0);
        WorldCapture.staticField(client.class,"cn",int.class,0);
        if(capture.game.getCameraFocusEntity()!=capture.game.getLocalPlayer())
            throw new IllegalStateException("Native camera focus is not the controlled original local player");
        playerPosition(capture.game.getLocalPlayer().getLocalLocation().getX(),
            capture.game.getLocalPlayer().getLocalLocation().getY());
        callbacks();
        for(lw[] group:host.hud.widgets.ax)
        {
            if(group==null)continue;
            for(lw widget:group)
                if(widget!=null && get(widget,lw.class,"ft",Object[].class)!=null)
                    System.out.println("CAMERA_SCROLL_LISTENER "+((net.runelite.api.widgets.Widget)widget).getId()+" "
                        +Arrays.toString((Object[])get(widget,lw.class,"ft",Object[].class)));
        }
        snapshot("controlled-normal-setup",null);
    }

    void callbacks()
    {
        ((client)capture.game).xr=(Callbacks)Proxy.newProxyInstance(Callbacks.class.getClassLoader(),
            new Class<?>[]{Callbacks.class},(proxy,method,args)->
            {
                if(method.getName().equals("mouseWheelMoved"))return args[0];
                if(method.getReturnType()==boolean.class)return false;
                if(method.getReturnType()==int.class)return 0;
                if(method.getReturnType()==long.class)return 0L;
                return null;
            });
    }

    void recordScript(int id) throws Exception
    {
        var index=capture.cache.store.findIndex(12);
        var archive=index.getArchive(id);
        if(archive==null)throw new IllegalStateException("Required native camera script absent: "+id);
        byte[] container=capture.cache.store.getStorage().loadArchive(archive);
        byte[] raw=archive.getFiles(Arrays.copyOf(container,container.length-2)).findFile(0).getContents();
        byte[] executed=capture.cache.archive(12).loadData(id,0);
        var decoder=new net.runelite.cache.definitions.loaders.ScriptLoader().configureForRevision(index.getRevision());
        scriptEvidence.put(id,OriginalCapture.map("source_index",12,"source_group",id,"source_file",0,
            "group_crc32",Integer.toUnsignedLong(archive.getCrc()),"group_revision",archive.getRevision(),
            "original_payload_sha256",OriginalCapture.hash(raw),"native_loaded_payload_sha256",OriginalCapture.hash(executed),
            "matching_runtime_overlay",!Arrays.equals(raw,executed),
            "original_definition",decoder.load(id,raw),"native_loaded_definition",decoder.load(id,executed)));
    }

    void playerPosition(int x,int y) throws Exception
    {
        Object player=capture.game.getLocalPlayer();
        WorldCapture.logicalInt(player,dh.class,"bb",-1547553299,x);
        WorldCapture.logicalInt(player,dh.class,"bi",-1272026483,y);
        put(player,dh.class,"ou",float.class,(float)x);
        put(player,dh.class,"ft",float.class,(float)y);
    }

    void snapshot(String label,Object input) throws Exception
    {
        Map<String,Object> state=cameraState(capture.game);
        state.put("label",label);
        state.put("input",input);
        state.put("step",step);
        state.put("player_local",capture.game.getLocalPlayer().getLocalLocation());
        state.put("world_base",new int[]{host.world.getBaseX(),host.world.getBaseY()});
        state.put("plane",host.world.getPlane());
        int x=capture.game.getLocalPlayer().getLocalLocation().getX();
        int y=capture.game.getLocalPlayer().getLocalLocation().getY();
        state.put("source_player_ground_height",host.world.getTileHeight(x,y,host.world.getPlane()));
        state.put("actor_input_scope","Controlled source actor logical/render coordinates; actor movement interpolation is not under test.");
        state.put("wheel_varbits",new int[]{capture.game.getVarbitValue(4606),capture.game.getVarbitValue(6357)});
        Map<Integer,Integer> settings=new LinkedHashMap<>();
        for(int id:new int[]{73,74,1338,1339,1340,1341})
            settings.put(id,capture.game.getVarcIntValue(id));
        state.put("camera_varc_ints",settings);
        state.put("source_actor_render_coordinates",new float[]{
            (float)get(capture.game.getLocalPlayer(),dh.class,"ou",float.class),
            (float)get(capture.game.getLocalPlayer(),dh.class,"ft",float.class)});
        state.put("source_actor_footprint_size",capture.game.getLocalPlayer().getFootprintSize());
        rows.add(state);
        System.out.println("CAMERA_STATE "+label+" step="+step+" target="+capture.game.getCameraPitchTarget()+","
            +capture.game.getCameraYawTarget()+" position="+Arrays.toString((int[])state.get("camera_x_height_y")));
    }

    void paint() throws Exception
    {
        if(capture.game.getCameraMode()!=0 || (boolean)get(null,client.class,"kb",boolean.class))
            throw new IllegalStateException("Probe is not in original normal camera mode");
        capture.target(width,height,0,512);
        qi.ck.az(width,height,host.hud.widgets,1,2,-293044276);
        dj writer=(dj)get(null,client.class,"ai",dj.class);
        if(writer.ap!=null)throw new IllegalStateException("Unexpected original network transport");
    }

    void input(int[] keys,int mouseButton,int mouseX,int mouseY) throws Exception
    {
        fa keyboard=(fa)get(null,client.class,"eb",fa.class);
        boolean[] pressed=(boolean[])get(keyboard,fa.class,"ad",boolean[].class);
        Arrays.fill(pressed,false);
        for(int key:keys)pressed[key]=true;
        integer(tz.class,"ab",2090434187,mouseButton);
        integer(tz.class,"ag",-38255113,mouseX);
        integer(tz.class,"as",-2144333897,mouseY);
    }

    void tick(String label,int[] keys,int button,int mx,int my) throws Exception
    {
        tick(label,keys,button,mx,my,0,20_000_000L);
    }

    void tick(String label,int[] keys,int button,int mx,int my,int wheel,long frameNanos) throws Exception
    {
        input(keys,button,mx,my);
        integer(client.class,"cm",1612595797,++step);
        int originalWheel=0;
        if(wheel!=0)
        {
            tc handler=new tc();
            handler.mouseWheelMoved(new MouseWheelEvent(new Canvas(),MouseWheelEvent.MOUSE_WHEEL,0L,0,mx,my,0,false,
                MouseWheelEvent.WHEEL_UNIT_SCROLL,1,wheel));
            originalWheel=handler.az(-946763283);
            if(originalWheel!=wheel || handler.az(-946763283)!=0)throw new IllegalStateException("Native wheel accumulator differs");
        }
        gb.co.as(host.hud.widgets,width,height,step,originalWheel,host.hud.context,(byte)1);
        gb.co.ad(host.hud.context,host.hud.widgets,(byte)-35);
        zj.be(1161256214);
        Map<String,Object> tickState=cameraState(capture.game);
        ((client)capture.game).ft(frameNanos);
        ((client)capture.game).dq(frameNanos);
        paint();
        snapshot(label,OriginalCapture.map("keys",keys,"native_mouse_button",button,"mouse",new int[]{mx,my},
            "wheel_rotation",wheel,"source_tick_period_ms",20,"controlled_render_frame_nanoseconds",frameNanos,
            "after_original_fixed_tick",tickState,
            "native_tick","pq.as/ad -> zj.be(1161256214)",
            "native_render_frame_update","client.ft -> client.dq; original client.ds camera functions, with explicitly controlled actor render coordinates"));
    }

    void run(String scenario) throws Exception
    {
        if(scenario.equals("script-init-scan"))
        {
            List<Object> found=new ArrayList<>();
            List<Object> shortInputs=new ArrayList<>();
            var index=capture.cache.store.findIndex(12);
            var loader=new net.runelite.cache.definitions.loaders.ScriptLoader().configureForRevision(index.getRevision());
            for(var archive:index.getArchives())
            {
                int id=archive.getArchiveId();
                byte[] container=capture.cache.store.getStorage().loadArchive(archive);
                byte[] bytes=archive.getFiles(Arrays.copyOf(container,container.length-2)).findFile(0).getContents();
                if(bytes.length<22)
                {
                    shortInputs.add(OriginalCapture.map("source_script_id",id,"size_bytes",bytes.length,
                        "sha256",OriginalCapture.hash(bytes),"payload_hex",java.util.HexFormat.of().formatHex(bytes),
                        "qualification","Short original source payload, below script header size; not interpreted as a camera initializer."));
                    continue;
                }
                var script=loader.load(id,bytes);
                for(int i=0;i<script.getInstructions().length;i++)
                {
                    int operand=script.getIntOperands()[i];
                    if((script.getInstructions()[i]==43 && (operand==1338 || operand==1339 || operand==1340 || operand==1341))
                        || (script.getInstructions()[i]==40 && (operand==605 || operand==604 || operand==626 || operand==603)))
                    {
                        found.add(OriginalCapture.map("source_script_id",id,"source_payload_sha256",OriginalCapture.hash(bytes),
                            "script",script));
                        System.out.println("CAMERA_INITIALIZER_SOURCE "+id+" varc="+operand);
                        break;
                    }
                }
            }
            Files.writeString(capture.output.resolve("camera-script-initializers.json"),OriginalCapture.JSON.toJson(found));
            Files.writeString(capture.output.resolve("short-script-inputs.json"),OriginalCapture.JSON.toJson(shortInputs));
            return;
        }
        tick("initial-follow",new int[]{},0,100,100);
        if(scenario.equals("keyboard"))
        {
            for(int i=0;i<12;i++)tick("left-held",new int[]{96},0,100,100);
            for(int i=0;i<8;i++)tick("released",new int[]{},0,100,100);
            for(int i=0;i<12;i++)tick("right-up-held",new int[]{97,98},0,100,100);
            for(int i=0;i<8;i++)tick("released",new int[]{},0,100,100);
        }
        if(scenario.equals("clamps"))
        {
            for(int i=0;i<70;i++)tick("pitch-up-to-clamp",new int[]{98},0,100,100);
            for(int i=0;i<100;i++)tick("pitch-down-to-clamp",new int[]{99},0,100,100);
            tick("opposed-arrows",new int[]{96,97,98,99},0,100,100);
            for(int i=0;i<8;i++)tick("release-all",new int[]{},0,100,100);
        }
        if(scenario.equals("mouse"))
        {
            WorldCapture.staticField(on.class,"hd",boolean.class,false);
            tick("middle-disabled-source-option",new int[]{},4,110,106);
            WorldCapture.staticField(on.class,"hd",boolean.class,true);
            tick("enabled-neutral",new int[]{},0,100,100);
            tick("middle-first",new int[]{},4,110,106);
            tick("middle-stationary",new int[]{},4,110,106);
            tick("middle-plus-one",new int[]{},4,111,107);
            tick("middle-reverse",new int[]{},4,100,100);
            for(int i=0;i<8;i++)tick("released",new int[]{},0,100,100);
        }
        if(scenario.equals("orbit"))
        {
            for(int pitch:new int[]{1024,1536,2048,3064})
            {
                for(int yaw:new int[]{0,4096,8192,12288,16383})
                {
                    capture.game.setCameraPitchTarget(pitch);
                    capture.game.setCameraYawTarget(yaw);
                    tick("controlled-orbit-"+pitch+"-"+yaw,new int[]{},0,100,100);
                }
            }
        }
        if(scenario.equals("follow"))
        {
            int x=capture.game.getLocalPlayer().getLocalLocation().getX();
            int y=capture.game.getLocalPlayer().getLocalLocation().getY();
            playerPosition(x+128,y);
            for(int i=0;i<16;i++)tick("player-one-tile-east",new int[]{},0,100,100);
            playerPosition(x+768,y);
            tick("controlled-teleport",new int[]{},0,100,100);
            playerPosition(x,y);
            tick("controlled-recenter",new int[]{},0,100,100);
        }
        if(scenario.equals("follow-threshold"))
        {
            int x=capture.game.getLocalPlayer().getLocalLocation().getX();
            int y=capture.game.getLocalPlayer().getLocalLocation().getY();
            for(int delta:new int[]{500,501,-500,-501})
            {
                WorldCapture.staticField(client.class,"ln",float.class,(float)x);
                WorldCapture.staticField(client.class,"bn",float.class,(float)y);
                integer(jy.class,"mg",318828117,x);
                integer(pf.class,"mk",68756747,y);
                playerPosition(x+delta,y);
                tick("follow-threshold-"+delta,new int[]{},0,100,100);
            }
        }
        if(scenario.equals("frame-rates"))
        {
            for(long nanos:new long[]{10_000_000L,20_000_000L,40_000_000L})
            {
                int x=6976,y=6464;
                WorldCapture.staticField(client.class,"ln",float.class,(float)x);
                WorldCapture.staticField(client.class,"bn",float.class,(float)y);
                integer(jy.class,"mg",318828117,x);
                integer(pf.class,"mk",68756747,y);
                capture.game.setCameraPitchTarget(1536);
                capture.game.setCameraYawTarget(0);
                integer(client.class,"jm",-1414186039,0);
                integer(client.class,"jh",-1891703075,0);
                WorldCapture.staticField(client.class,"tt",int.class,0);
                WorldCapture.staticField(client.class,"cn",int.class,0);
                playerPosition(x+128,y);
                tick("render-frame-"+nanos,new int[]{97},0,100,100,0,nanos);
            }
        }
        if(scenario.equals("resize"))
        {
            for(int[] size:new int[][]{{1024,768},{1920,1080},{2560,1440},{1920,1080}})
            {
                width=size[0];height=size[1];
                integer(sa.class,"qy",773246731,width);
                integer(eu.class,"qx",8379747,height);
                cn.ae(161,width,height,false,host.hud.widgets,host.hud.context,(short)217);
                capture.game.runScript(907,161<<16,1130);
                tick("native-resize-"+width+"x"+height,new int[]{},0,100,100);
            }
        }
        if(scenario.equals("terrain"))
        {
            for(int[] tile:new int[][]{{3222,3218},{3219,3218},{3215,3218},{3208,3218},{3218,3224}})
            {
                playerPosition((tile[0]-host.baseX)*128+64,(tile[1]-host.baseY)*128+64);
                for(int i=0;i<4;i++)tick("source-ground-"+tile[0]+","+tile[1],new int[]{},0,100,100);
            }
        }
        if(scenario.equals("region-shift"))
        {
            WorldCapture.staticField(bi.class,"kv",sm.class,new sm(kf.kw));
            rl4 loader=(rl4)get(null,client.class,"rs",rl4.class);
            int originalX=host.world.getBaseX(),originalY=host.world.getBaseY();
            WorldCapture.staticField(client.class,"kb",boolean.class,true);
            snapshot("before-original-region-rebase",null);
            loader.wb=originalX+8;
            loader.za=originalY+8;
            client.yk(loader);
            snapshot("after-original-region-rebase",OriginalCapture.map("entrypoint","client.yk(rl4)",
                "controlled_base_delta_tiles",new int[]{8,8},"scope","Original in-memory region transition, not an observed login/arrival packet."));
            loader.wb=originalX;
            loader.za=originalY;
            client.yk(loader);
            snapshot("after-original-region-rebase-return",OriginalCapture.map("controlled_base_delta_tiles",new int[]{-8,-8}));
        }
        if(scenario.equals("zoom-separation"))
        {
            int[] values={256,320,320,384};
            for(int i=0;i<values.length;i+=2)
            {
                bb.ay[0]=values[i];bb.ay[1]=values[i+1];
                integer(dy.class,"aq",-324749371,2);
                int result=lo.bv(6201,null,false,-1503643048);
                if(result!=1)throw new IllegalStateException("Original viewport-distance opcode unavailable");
                tick("native-distance-scale-"+values[i]+"-"+values[i+1],new int[]{},0,100,100);
            }
        }
        if(scenario.equals("wheel"))
        {
            for(int id:new int[]{39,42,603,604,605,626,1045,1046,1049})recordScript(id);
            var script=new net.runelite.cache.definitions.loaders.ScriptLoader()
                .configureForRevision(capture.cache.store.findIndex(12).getRevision())
                .load(39,capture.cache.archive(12).loadData(39,0));
            Files.writeString(capture.output.resolve("wheel-script39.json"),OriginalCapture.JSON.toJson(script));
            var nextScript=new net.runelite.cache.definitions.loaders.ScriptLoader()
                .configureForRevision(capture.cache.store.findIndex(12).getRevision())
                .load(42,capture.cache.archive(12).loadData(42,0));
            Files.writeString(capture.output.resolve("wheel-script42.json"),OriginalCapture.JSON.toJson(nextScript));
            capture.game.runScript(626);
            snapshot("original-source-626-bounds-initialized",OriginalCapture.map(
                "source_script",626,"qualification","Original fixed bound initializer; startup invocation provenance is separately traced."));
            capture.game.runScript(42,512,512);
            tick("controlled-source-zoom-512",new int[]{},0,960,540);
            for(int i=0;i<5;i++)tick("viewport-wheel-out",new int[]{},0,960,540,1,20_000_000L);
            for(int i=0;i<5;i++)tick("viewport-wheel-in",new int[]{},0,960,540,-1,20_000_000L);
            tick("sidebar-wheel",new int[]{},0,1820,850,1,20_000_000L);
            var widget=capture.game.getWidget(161,1);
            Object[] listener=(Object[])get(widget,lw.class,"ft",Object[].class);
            if(listener==null || (int)listener[0]!=39)throw new IllegalStateException("Original camera wheel listener changed");
            for(int i=0;i<3;i++)
            {
                zs event=(zs)capture.game.createScriptEventBuilder(listener).setSource(widget);
                event.ab(1,(byte)1);
                var built=((net.runelite.api.ScriptEventBuilder)event).build();
                System.out.println("CAMERA_WHEEL_EVENT mouseY="+built.getMouseY()+" varbits="
                    +capture.game.getVarbitValue(4606)+","+capture.game.getVarbitValue(6357));
                built.run();
                tick("original-wheel-event-delivery",new int[]{},0,960,540);
            }
            capture.game.runScript(39,1);
            tick("original-script39-direct",new int[]{},0,960,540);
            tick("wheel-to-lower-bound",new int[]{},0,960,540,100,20_000_000L);
            tick("wheel-to-upper-bound",new int[]{},0,960,540,-100,20_000_000L);
            capture.game.setVarbit(4606,1);
            tick("source-wheel-disabled-varbit",new int[]{},0,960,540,1,20_000_000L);
        }
    }

    public static void main(String[] args)
    {
        try
        {
            if(args.length!=4 || !Arrays.asList(SCENARIOS).contains(args[2]))
                throw new IllegalArgumentException("Expected cache, private scratch, admitted camera scenario and trace output");
            Field random=Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
            random.setAccessible(true);
            ((java.util.Random)random.get(null)).setSeed(0L);
            client first=new client();
            oe.cz=first;
            first.km=Thread.currentThread();
            Map<String,Object> defaults=cameraState(first);
            try(OriginalCache cache=new OriginalCache(Path.of(args[0])))
            {
                CameraProbe probe=new CameraProbe(cache,Path.of(args[1]),defaults);
                probe.run(args[2]);
                Files.writeString(Path.of(args[3]),OriginalCapture.JSON.toJson(OriginalCapture.map(
                    "schema_version",1,"scenario",args[2],"classification","Controlled offline original normal-camera method observations",
                    "constructor_defaults",defaults,"traces",probe.rows,"native_indexes",cache.provenance,
                    "camera_script_evidence",probe.scriptEvidence,
                    "physical_arrow_mapping",OriginalCapture.map("mapping","original tk.dr Java keycode translation",
                        "left_java37",((int[])get(null,tk.class,"dr",int[].class))[37],
                        "right_java39",((int[])get(null,tk.class,"dr",int[].class))[39],
                        "up_java38",((int[])get(null,tk.class,"dr",int[].class))[38],
                        "down_java40",((int[])get(null,tk.class,"dr",int[].class))[40]),
                    "controlled_start_scope","Original constructor values plus explicitly supplied focus entity/actor coordinates; no Tutorial or Lumbridge account camera default is inferred.",
                    "native_logic_cycle_period_ns",20_000_000,
                    "timing_source","Unchanged tq.run dispatches mh.bs(20,1); original client.ds divides native frame elapsed nanoseconds by2e7 and calls ft/dq.",
                    "authenticated_source_initial_state",false,"server_camera_defaults_established",false,
                    "network_pump",false,"original_jar_modified",false)));
            }
            System.exit(0);
        }
        catch(Throwable error)
        {
            error.printStackTrace();
            System.exit(1);
        }
    }
}
