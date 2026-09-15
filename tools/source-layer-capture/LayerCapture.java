import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;
import net.runelite.api.DynamicObject;
import net.runelite.api.ItemLayer;
import net.runelite.api.Model;
import net.runelite.api.Renderable;
import net.runelite.api.Tile;
import net.runelite.api.TileItem;
import net.runelite.api.WallObject;
import net.runelite.api.widgets.Widget;

/** Supplementary controlled states, rendered entirely by the unchanged original client. */
public final class LayerCapture
{
    final OriginalCapture capture;
    final JsonObject input;
    final String id;
    HudCapture hud;
    dz world;
    int baseX, baseY, cameraX, cameraY, cameraZ, focalX, focalY;
    final List<Object> operations = new ArrayList<>();
    final List<Object> quality = new ArrayList<>();
    final Map<String,Object> sourceInputs = new LinkedHashMap<>();
    dy pinnedFire;
    int fireFrame = -1;

    LayerCapture(OriginalCapture capture, JsonObject input)
    {
        this.capture = capture;
        this.input = input;
        this.id = input.get("id").getAsString();
    }

    static Object field(Object target, Class<?> owner, String name, Class<?> type) throws Exception
    {
        for (Field field : owner.getDeclaredFields())
        {
            if (field.getName().equals(name) && field.getType() == type)
            {
                field.setAccessible(true);
                return field.get(target);
            }
        }
        throw new IllegalStateException("Missing exact native field " + owner + "." + name);
    }

    static Object call(Object target, Class<?> owner, String name, Class<?>[] types, Object... values) throws Exception
    {
        Method method = owner.getDeclaredMethod(name, types);
        method.setAccessible(true);
        return method.invoke(target, values);
    }

    static void set(Object target, Class<?> owner, String name, Class<?> type, Object value) throws Exception
    {
        for (Field field : owner.getDeclaredFields())
        {
            if (field.getName().equals(name) && field.getType() == type)
            {
                field.setAccessible(true);
                field.set(target, value);
                return;
            }
        }
        throw new IllegalStateException("Missing exact mutable native field " + owner + "." + name);
    }

    void world() throws Exception
    {
        hud = new HudCapture(capture);
        WorldCapture helper = new WorldCapture(capture);
        call(helper, WorldCapture.class, "initializeVariables", new Class<?>[]{});
        fq.ab(32768);
        ez.cc(25);
        boolean tutorial = input.get("camera").getAsString().equals("tutorial-starting-house");
        baseX = tutorial ? 3048 : 3168;
        baseY = tutorial ? 3056 : 3168;
        int cx = tutorial ? 3094 : 3222, cy = tutorial ? 3095 : 3208;
        int fx = tutorial ? 3094 : 3222, fy = tutorial ? 3103 : 3218;
        call(helper, WorldCapture.class, "view",
            new Class<?>[]{String.class, int.class, int.class, int.class, int.class, int.class,
                int.class, int.class, int.class, boolean.class},
            id, baseX, baseY, cx, tutorial ? -1000 : -1300, cy, fx, fy, 0, false);
        world = is.dk;
        cameraX = (cx - baseX) * 128;
        cameraZ = (cy - baseY) * 128;
        focalX = (fx - baseX) * 128;
        focalY = (fy - baseY) * 128;
        cameraY = world.al[0][fx - baseX][fy - baseY] + (tutorial ? -1000 : -1300);
        WorldCapture.logicalInt(null, client.class, "cm", 1612595797, 0);
        WorldCapture.logicalInt(world, dz.class, "ag", -483624883, input.get("plane").getAsInt());
        world.ae.setDrawDistance(25);
        world.ae.setRoofRemovalMode(0);
        set(ab.kn, cy.class, "as", boolean.class, input.get("hide_roofs").getAsBoolean());
        if (input.get("layer").getAsString().equals("mapped-chunk")) mappedChunk();
        for (int region : world.zn)
        {
            source(5,region,0);
            source(5,region,1);
        }
    }

    void hud() throws Exception
    {
        client.gc(-1);
        WorldCapture.staticField(lb.class, "az", int[].class, capture.game.getVarps().clone());
        call(hud, HudCapture.class, "player", new Class<?>[]{});
        int px = input.getAsJsonArray("player_tile").get(0).getAsInt();
        int py = input.getAsJsonArray("player_tile").get(1).getAsInt();
        WorldCapture.logicalInt(capture.game.getLocalPlayer(), dh.class, "bb", -1547553299, (px - baseX) * 128 + 64);
        WorldCapture.logicalInt(capture.game.getLocalPlayer(), dh.class, "bi", -1272026483, (py - baseY) * 128 + 64);
        WorldCapture.logicalInt(null, client.class, "np", 2106329293, (px - baseX) * 128 + 64);
        WorldCapture.logicalInt(null, client.class, "nq", -2126074583, (py - baseY) * 128 + 64);
        call(hud, HudCapture.class, "inventory", new Class<?>[]{});
        int[] quests = HudSourceCatalog.write(capture.cache, capture.output);
        for (int[] varbit : new int[][]{{4609,1},{5605,1},{8119,1},{357,17},{11877,quests[0]},{1782,quests[1]},{6347,0}})
            capture.game.setVarbit(varbit[0], varbit[1]);
        WorldCapture.logicalInt(null, ba.class, "ao", -782895767, 0);
        minimap();
        capture.target(1920,1080,0,512);
        call(hud, HudCapture.class, "load", new Class<?>[]{int.class,boolean.class},161,false);
        hud.context = (qn) field(null,client.class,"ca",qn.class);
        if (hud.context == null) throw new IllegalStateException("Missing original widget context");
        cn.ae(161,1920,1080,false,hud.widgets,hud.context,(short)217);
        call(hud,HudCapture.class,"load",new Class<?>[]{int.class,boolean.class},161,true);
        for (int[] binding : new int[][]{{96,162},{33,160},{76,593},{77,320},{78,399},{79,149},{80,387},
            {81,541},{82,218},{83,707},{84,109},{85,429},{86,182},{87,116},{88,216},{89,239}})
            capture.game.openInterface(161 << 16 | binding[0],binding[1],1);
        call(hud,HudCapture.class,"selectTab",new Class<?>[]{int.class},3);
    }

    void minimap() throws Exception
    {
        call(hud,HudCapture.class,"minimap",new Class<?>[]{});
        ym map = new ym(512,512);
        client.bm(world,map,4.0,world.getPlane(),0,0,48,48);
        WorldCapture.staticField(rd.class,"ax",ym.class,map);
        operations.add(OriginalCapture.map("entrypoint","client.bm","plane",world.getPlane(),
            "source_scene",true,"scale",4.0,"canvas",new int[]{512,512},
            "pixel_hash_format","native int32 big-endian, exactly SpritePixels.getPixels()",
            "minimap_pixel_sha256",pixelsHash(map.getPixels())));
    }

    static String pixelsHash(int[] pixels) throws Exception
    {
        ByteBuffer bytes = ByteBuffer.allocate(pixels.length * 4);
        for (int pixel : pixels) bytes.putInt(pixel);
        return OriginalCapture.hash(bytes.array());
    }

    Tile tile(int x, int y, int plane)
    {
        Tile result = ((net.runelite.api.Scene)world.ae).getTiles()[plane][x-baseX][y-baseY];
        if (result == null) throw new IllegalStateException("Missing source tile "+x+","+y+","+plane);
        return result;
    }

    void door() throws Exception
    {
        WallObject prior = tile(3098,3107,0).getWallObject();
        if (prior == null || prior.getId()!=9398 || (prior.getConfig() & 31)!=0)
            throw new IllegalStateException("Original starting door placement differs");
        int orientation = input.get("orientation").getAsInt();
        if (orientation != 0)
            mi.ey(world,0,0,3098-baseX,3107-baseY,9398,orientation,0,-1,(byte)0);
        WallObject after = tile(3098,3107,0).getWallObject();
        if (after == null || after.getId()!=9398 || (after.getConfig() & 31)!=0
            || ((after.getConfig() >> 6)&3)!=orientation)
            throw new IllegalStateException("Original door update did not preserve ID/type/orientation");
        source(2,6,9398);
        source(7,9476,0);
        operations.add(OriginalCapture.map("entrypoint","mi.ey -> fk.at -> ez.br","object_id",9398,
            "source_tile",new int[]{3098,3107,0},"shape",0,"model_id",9476,"orientation",orientation,
            "orientation_a",after.getOrientationA(),"world_position",after.getWorldLocation(),
            "native_definition_actions",capture.game.getObjectDefinition(9398).getActions(),
            "native_config",after.getConfig(),"native_draw_anchor_x_height_y",new int[]{after.getX(),after.getZ(),after.getY()},
            "geometry",geometry(nativeModel(after.getRenderable1())),
            "qualification","The native source wall is rotated in controlled state; no authenticated server open packet/hinge observation claimed."));
    }

    void ground() throws Exception
    {
        for (var entry : input.getAsJsonArray("items"))
        {
            int item = entry.getAsJsonArray().get(0).getAsInt(), quantity = entry.getAsJsonArray().get(1).getAsInt();
            sourceItem(item,quantity);
            bu.dg(world,0,3221-baseX,3217-baseY,item,quantity,31,0,600,0,false,(byte)0);
        }
        ItemLayer layer = tile(3221,3217,0).getItemLayer();
        if (layer == null) throw new IllegalStateException("Native ground-item pile absent");
        List<Object> selected = new ArrayList<>();
        Renderable[] renderables = {layer.getBottom(),layer.getMiddle(),layer.getTop()};
        String[] names = {"bottom","middle","top"};
        for (int i=0;i<renderables.length;i++)
        {
            if (renderables[i] == null) continue;
            TileItem item = (TileItem) renderables[i];
            selected.add(OriginalCapture.map("slot",names[i],"item_id",item.getId(),"quantity",item.getQuantity(),
                "geometry",geometry(item.getModel())));
        }
        operations.add(OriginalCapture.map("entrypoint","bu.dg -> lj.es -> native item pile","tile",new int[]{3221,3217,0},
            "world_position",layer.getWorldLocation(),"local_position",layer.getLocalLocation(),
            "source_z",layer.getZ(),"support_height",layer.getHeight(),
            "native_draw_anchor_x_height_y",new int[]{layer.getX(),layer.getZ()-layer.getHeight(),layer.getY()},
            "drawpoint_scope","All selected original Renderables use this native pile anchor; their source vertex offsets remain unmodified.",
            "selected",selected));
    }

    void fire() throws Exception
    {
        mi.ey(world,0,2,3221-baseX,3217-baseY,26185,0,10,-1,(byte)0);
        net.runelite.api.GameObject placed = null;
        for (var object : tile(3221,3217,0).getGameObjects())
            if (object != null && object.getId()==26185) placed = object;
        if (placed==null) throw new IllegalStateException("Native dynamic fire placement absent");
        pinnedFire = new dy(world,26185,10,0,0,3221-baseX,3217-baseY,475,false,null);
        set(placed,fb.class,"az",ee.class,pinnedFire);
        WorldCapture.logicalInt(pinnedFire,dy.class,"ao",1618438999,0);
        DynamicObject fire = pinnedFire;
        int frame=input.get("animation_frame").getAsInt();
        Model model=fire.getModel();
        int phaseCycles = 0;
        while (fire.getAnimFrame()!=frame && phaseCycles<30)
        {
            WorldCapture.logicalInt(null,client.class,"cm",1612595797,++phaseCycles);
            model=fire.getModel();
        }
        if (fire.getAnimFrame()!=frame) throw new IllegalStateException("Native fire did not reach requested source frame");
        fireFrame=frame;
        WorldCapture.logicalInt(null,client.class,"cm",1612595797,0);
        WorldCapture.logicalInt(pinnedFire,dy.class,"ao",1618438999,0);
        source(2,6,26185);
        source(7,2260,0);
        var sourceSequence=new net.runelite.cache.definitions.loaders.SequenceLoader()
            .configureForRevision(capture.cache.store.findIndex(2).getArchive(12).getRevision())
            .load(475,source(2,12,475));
        for(int packed:sourceSequence.frameIDs)
        {
            byte[] bytes=source(0,packed>>>16,packed&65535);
            source(1,((bytes[0]&255)<<8)|(bytes[1]&255),0);
        }
        operations.add(OriginalCapture.map("entrypoint","mi.ey -> fk.at -> dy -> original qr animation",
            "object_id",26185,"tile",new int[]{3221,3217,0},"model_id",2260,"sequence_id",475,
            "requested_frame",frame,"native_frame",fire.getAnimFrame(),"native_anim_cycle_api",fire.getAnimCycle(),
            "native_animation_advance_cycles",phaseCycles,"scene_client_cycle",capture.game.getGameCycle(),
            "randomize_initial_phase",false,"frame_lengths",fire.getAnimation().getFrameLengths(),
            "phase_control","Original dy constructor random=false, original dy.rf/ou.pl stepping, then last-update cycle rebased to zero to keep all other source scenery at its shared phase.",
            "geometry",geometry(model)));
    }

    void mappedChunk() throws Exception
    {
        xk request = new xk();
        request.az = true;
        request.af = world.zn.clone();
        for (int plane=0;plane<4;plane++)
            for(int x=0;x<13;x++)
                for(int y=0;y<13;y++)
                    request.ae[plane][x][y]=(plane<<24)|((baseX/8+x)<<14)|((baseY/8+y)<<3);
        request.ae[0][6][6] |= 2 << 1;
        world.bp = request.ae;
        world.gi = true;
        rl4 loader = new rl4(null,0,world,request);
        loader.wb=baseX; loader.za=baseY;
        loader.xo=baseX/8+6; loader.yw=baseY/8+6; loader.xn=0;
        WorldCapture.staticField(client.class,"rs",rl4.class,loader);
        if (!loader.fn()) throw new IllegalStateException("Native mapped chunk loader has unavailable original input");
        world.ae=loader.gb; world.al=loader.nw; world.aj=loader.zw;
        set(world.ae,ez.class,"jd",dz.class,world);
        world.ae.setDrawDistance(25);
        world.ae.setRoofRemovalMode(0);
        if (!world.isInstance() || world.getInstanceTemplateChunks()[0][6][6]!=request.ae[0][6][6])
            throw new IllegalStateException("Native instance mapping readback differs");
        operations.add(OriginalCapture.map("entrypoint","Original rl4.fn instanced-chunk decode/placement",
            "qualification","Controlled source chunk mapping, not an observed live instance or gameplay route.",
            "all_other_chunks","Identity mappings for all four planes of the 104x104 source envelope",
            "target_local_chunk",new int[]{0,6,6},"source_chunk",new int[]{0,baseX/8+6,baseY/8+6},
            "rotation_quarter_turns",2,"packed_template",request.ae[0][6][6],
            "source_units_per_tile",128));
    }

    Object geometry(Model model) throws Exception
    {
        if (model==null) throw new IllegalStateException("Native source model is null");
        @SuppressWarnings("unchecked")
        Map<String,Object> result=(Map<String,Object>)call(capture,OriginalCapture.class,"geometry",new Class<?>[]{Model.class},model);
        ByteBuffer faces=ByteBuffer.allocate(model.getFaceCount()*12);
        for(int i=0;i<model.getFaceCount();i++)
        {
            faces.putInt(model.getFaceIndices1()[i]);
            faces.putInt(model.getFaceIndices2()[i]);
            faces.putInt(model.getFaceIndices3()[i]);
        }
        result.put("triangle_indices_int32_be_sha256",OriginalCapture.hash(faces.array()));
        return result;
    }

    byte[] source(int index,int group,int file) throws Exception
    {
        byte[] payload=capture.cache.archive(index).loadData(group,file);
        if(payload==null || payload.length==0)throw new IllegalStateException("Missing exact source input "+index+"/"+group+"/"+file);
        var archive=capture.cache.store.findIndex(index).getArchive(group);
        byte[] container=capture.cache.store.getStorage().loadArchive(archive);
        sourceInputs.put(index+"/"+group+"/"+file,OriginalCapture.map("archive",index,"group",group,"file",file,
            "size_bytes",payload.length,"sha256",OriginalCapture.hash(payload),
            "group_crc32",Integer.toUnsignedLong(archive.getCrc()),"group_full_revision",archive.getRevision(),
            "disk_container_size_bytes",container.length,"disk_container_sha256",OriginalCapture.hash(container)));
        return payload;
    }

    void sourceItem(int id,int quantity) throws Exception
    {
        var loader=new net.runelite.cache.definitions.loaders.ItemLoader();
        var definition=loader.load(id,source(2,10,id));
        int selected=id;
        if(quantity>1 && definition.countObj!=null)
        {
            for(int i=0;i<definition.countObj.length;i++)
                if(definition.countCo[i]>0 && quantity>=definition.countCo[i])selected=definition.countObj[i];
        }
        var modelDefinition=selected==id?definition:loader.load(selected,source(2,10,selected));
        source(7,modelDefinition.inventoryModel,0);
        operations.add(OriginalCapture.map("source_item_id",id,"quantity",quantity,
            "native_count_variant_definition",selected,"inventory_model",modelDefinition.inventoryModel,
            "cost",definition.cost,"stackable_mode",definition.stackable,
            "role","Source identities for the native quantity/model choice, not an icon substitute or loot grant."));
    }

    Object census() throws Exception
    {
        int tiles=0,paints=0,tileModels=0,walls=0,ground=0,objectRefs=0,vertices=0,faces=0;
        java.util.Set<net.runelite.api.GameObject> objects=java.util.Collections.newSetFromMap(new java.util.IdentityHashMap<>());
        for(Tile[][] plane:((net.runelite.api.Scene)world.ae).getExtendedTiles())
            for(Tile[] column:plane)
                for(Tile tile:column)
                {
                    if(tile==null)continue;
                    tiles++;
                    if(tile.getSceneTilePaint()!=null)paints++;
                    if(tile.getSceneTileModel()!=null)
                    {
                        tileModels++;
                        vertices+=tile.getSceneTileModel().getVertexX().length;
                        faces+=tile.getSceneTileModel().getFaceX().length;
                    }
                    if(tile.getWallObject()!=null)walls++;
                    if(tile.getGroundObject()!=null)ground++;
                    for(var object:tile.getGameObjects())
                        if(object!=null){objects.add(object);objectRefs++;}
                }
        if(tiles<10000 || objectRefs<1000)throw new IllegalStateException("Incomplete native scenery census");
        return OriginalCapture.map("tiles",tiles,"tile_paints",paints,"tile_models",tileModels,
            "tile_model_vertices",vertices,"tile_model_faces",faces,"walls",walls,"ground_decorations",ground,
            "game_object_tile_references",objectRefs,"unique_game_objects",objects.size(),
            "scope","Original loaded scene data including native surrounding scenery, not a visible-triangle/performance claim.");
    }

    int decodedTint(String name,int writeMultiplier) throws Exception
    {
        int decoder=java.math.BigInteger.valueOf(Integer.toUnsignedLong(writeMultiplier))
            .modInverse(java.math.BigInteger.ONE.shiftLeft(32)).intValue();
        return (int)field(null,di.class,name,int.class)*decoder;
    }

    Model nativeModel(Renderable renderable)
    {
        if (renderable instanceof Model) return (Model)renderable;
        Model model = renderable.getModel();
        if (model == null) throw new IllegalStateException("No native model from "+renderable.getClass().getName());
        return model;
    }

    void probe() throws Exception
    {
        for (int n=9396;n<=9403;n++)
        {
            var definition = capture.game.getObjectDefinition(n);
            System.out.println("PROBE_DOOR "+n+" "+definition.getName()+" "+Arrays.toString(definition.getActions()));
        }
        System.out.println("PROBE_WORLD "+world.getBaseX()+","+world.getBaseY()+" plane="+world.getPlane());
        System.out.println("PROBE_TILEFLAGS inside="+world.aj[0][3094-baseX][3103-baseY]+" outside="+world.aj[0][3094-baseX][3099-baseY]);
    }

    void render() throws Exception
    {
        if(capture.game.isGpu())throw new IllegalStateException("Non-stock GPU rendering is not permitted in this reference host");
        int[] pixels=capture.target(1920,1080,0,512);
        gb.co.as(hud.widgets,1920,1080,1,0,hud.context,(byte)1);
        qi.ck.az(1920,1080,hud.widgets,1,2,-293044276);
        if (pinnedFire != null && pinnedFire.getAnimFrame()!=fireFrame)
            throw new IllegalStateException("Native fire frame changed during capture");
        int[] camera={capture.game.getCameraX(),capture.game.getCameraZ(),capture.game.getCameraY()};
        if (!Arrays.equals(camera,new int[]{cameraX,cameraY,cameraZ}))
            throw new IllegalStateException("Frozen source camera changed: "+Arrays.toString(camera));
        int zoom=(int)field(null,client.class,"fk",int.class)*1129651895;
        if(zoom!=410 || capture.game.getCameraPitch()!=2048 || capture.game.getCameraYaw()!=0)
            throw new IllegalStateException("Frozen native camera projection changed: zoom="+zoom
                +" pitch="+capture.game.getCameraPitch()+" yaw="+capture.game.getCameraYaw());
        List<Object> regions=new ArrayList<>();
        for (int component : new int[]{95,96,97})
            regions.add(call(hud,HudCapture.class,"region",new Class<?>[]{String.class,Widget.class,int[].class},
                "root161:"+component,capture.game.getWidget(161,component),pixels));
        regions.add(call(hud,HudCapture.class,"region",new Class<?>[]{String.class,Widget.class,int[].class},
            "inventory-panel",capture.game.getWidget(149,0),pixels));
        int sceneDrawPlane=(int)field(world.ae,ez.class,"br",int.class);
        int selective=(int)call(null,cz.class,"ch",new Class<?>[]{int.class},927500664);
        operations.add(OriginalCapture.map("entrypoint","cz.ch","normal_camera_stock_plane_selector",selective,
            "actual_locked_camera_draw_plane",sceneDrawPlane,
            "hide_roofs",input.get("hide_roofs").getAsBoolean(),"roof_removal_plugin_mode",world.ae.getRoofRemovalMode(),
            "player_source_tile_setting",world.aj[world.getPlane()][input.getAsJsonArray("player_tile").get(0).getAsInt()-baseX]
                [input.getAsJsonArray("player_tile").get(1).getAsInt()-baseY],
            "camera_source_tile_setting",world.aj[world.getPlane()][cameraX/128][cameraZ/128],
            "qualification","Actual image uses the unchanged approved locked camera and its native cz roof branch. cz.ch is separately evaluated original normal-camera selection, not a claim that this branch painted the image."));
        dj writer=(dj)field(null,client.class,"ai",dj.class);
        if(writer.ap!=null)throw new IllegalStateException("Unexpected original-client network transport");
        Map<Integer,Integer> varps=new java.util.TreeMap<>();
        for(int i=0;i<capture.game.getVarps().length;i++)
            if(capture.game.getVarps()[i]!=0)varps.put(i,capture.game.getVarps()[i]);
        List<Object> containers=new ArrayList<>();
        for(int container:new int[]{93,94})
            containers.add(OriginalCapture.map("id",container,"items",capture.game.getItemContainer(container).getItems()));
        int hueOffset=decodedTint("at",2133726259), lightnessOffset=decodedTint("an",-93161891);
        if(Math.abs(hueOffset)>8 || Math.abs(lightnessOffset)>16)
            throw new IllegalStateException("Native terrain tint is outside original bounds");
        capture.save("frames/"+id,pixels,1920,1080,0,"original-runtime-dynamic-layer",
            OriginalCapture.map("classification","controlled offline original-client rendering","case",input,
                "native_operations",operations,"native_indexes",capture.cache.provenance,
                "layer_source_inputs",sourceInputs,"scene_census",census(),"source_map_regions",world.zn,
                "native_pipeline","rl4.fn -> original widget update pq.as/paint gp.az -> cz.az -> ez.dh; native client.bm minimap",
                "controlled_containers",containers,"controlled_nonzero_varps",varps),
            OriginalCapture.map("layout","Original Resizable-Classic group161","dpr",1,"ui_scale",1,
                "camera_reference",input.get("camera").getAsString(),"camera_local_units",camera,"pitch",2048,"yaw",0,
                "dpr_scope","Native framebuffer pixel ratio1; no browser/DPR observation is claimed.",
                "zoom",zoom,"native_framebuffer",new int[]{1920,1080},"focal_local_units",new int[]{focalX,focalY},
                "zoom_scope","Native full-HUD viewport recalculates zoom410. The earlier viewport-only source fixture uses662; no viewport-zoom override is applied.",
                "camera_height_scope","Approved source-ground-relative eye height retained even when controlled plane or chunk mapping changes.",
                "angle_units_per_turn",16384,"brightness",0.8,"draw_distance",25,"texture_resolution",128,
                "far_clip_units",32768,"game_cycle",capture.game.getGameCycle(),"source_plane",world.getPlane(),
                "native_terrain_hue_offset",hueOffset,"native_terrain_lightness_offset",lightnessOffset,
                "maploader_invocations",input.get("layer").getAsString().equals("mapped-chunk")?2:1,
                "randomness_scope","Math.random seed0 per isolated case; native map-loading HSL offsets retained and read back. The mapped-chunk case includes its second original loader pass, not a normalized replacement palette.",
                "native_ui_regions",regions,"source_scene_draw_plane",sceneDrawPlane,
                "network_transport_connected",false,"source_gameplay_observed",false),20000);
        var record=OriginalCapture.map("id",id,"input",input,"capture",capture.captures.get(0),
            "quality_findings",quality,"candidate_compared",false,"new_presentation_approval",false);
        Files.writeString(capture.output.resolve(id+".json"),OriginalCapture.JSON.toJson(record));
        System.out.println("LAYER_CASE "+id+" scene_plane="+sceneDrawPlane+" selector="+selective);
    }

    public static void main(String[] args)
    {
        try
        {
            Field randomField=Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
            randomField.setAccessible(true);
            ((java.util.Random)randomField.get(null)).setSeed(0L);
            ImageIO.setUseCache(false);
            JsonObject contract=new JsonParser().parse(Files.readString(Path.of(args[2]))).getAsJsonObject();
            JsonObject selected=null;
            for (var value:contract.getAsJsonArray("cases"))
                if (value.getAsJsonObject().get("id").getAsString().equals(args[3])) selected=value.getAsJsonObject();
            if(selected==null)throw new IllegalArgumentException("Unknown case "+args[3]);
            try(OriginalCache cache=new OriginalCache(Path.of(args[0])))
            {
                LayerCapture layer=new LayerCapture(new OriginalCapture(cache,Path.of(args[1])),selected);
                layer.world();
                if(args[4].equals("probe")) layer.probe();
                else
                {
                    layer.hud();
                    String kind=selected.get("layer").getAsString();
                    if(kind.equals("door"))layer.door();
                    else if(kind.equals("ground"))layer.ground();
                    else if(kind.equals("fire"))layer.fire();
                    layer.minimap();
                    layer.render();
                }
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
