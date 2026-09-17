import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.lang.reflect.Field;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import javax.imageio.ImageIO;
import net.runelite.api.Animation;
import net.runelite.api.DecorativeObject;
import net.runelite.api.DynamicObject;
import net.runelite.api.GameObject;
import net.runelite.api.Renderable;
import net.runelite.api.Scene;
import net.runelite.api.Tile;
import net.runelite.api.TileObject;

/** Read-only animation observations around the unchanged original reference replay. */
public final class SceneryPhaseProbe
{
    private static final int[][] TARGETS = {{24969,3095,3102,0}, {196,3096,3105,0}, {196,3096,3110,0}};
    private final LayerCapture layer;
    private final List<Object> observations = new ArrayList<>();

    private SceneryPhaseProbe(LayerCapture layer) { this.layer = layer; }

    private static int raw(Object owner, Class<?> type, String name) throws Exception
    {
        return (int)LayerCapture.field(owner,type,name,int.class);
    }

    private Map<String,Object> controller(Object value) throws Exception
    {
        int frame = raw(value,qr.class,"ab");
        int offset = raw(value,qr.class,"as");
        int sequence = raw(value,qr.class,"ag");
        Animation animation = (Animation)LayerCapture.field(value,qr.class,"ae",ou.class);
        return OriginalCapture.map(
            "native_type",value.getClass().getName(),
            "frame_raw",frame,"frame_decode_multiplier",292569817,"frame",frame*292569817,
            "frame_cycle_raw",offset,"frame_cycle_decode_multiplier",-1399668821,
            "frame_cycle",offset*-1399668821,
            "sequence_raw",sequence,"sequence_decode_multiplier",1684838611,
            "sequence_id",sequence*1684838611,
            "sequence_api_id",animation==null?null:animation.getId(),
            "frame_lengths",animation==null?null:animation.getFrameLengths(),
            "other_clock_fields_raw",OriginalCapture.map("ax",raw(value,qr.class,"ax"),"af",raw(value,qr.class,"af")),
            "other_clock_fields_qualification","Raw values only; no additional clock semantics assigned.");
    }

    private Object placement(String phase, TileObject object, Renderable renderable, String slot) throws Exception
    {
        if (!(renderable instanceof DynamicObject) || !(renderable instanceof dy))
            throw new IllegalStateException("Requested animated source placement is not original dy: "+object.getId()+"/"+slot);
        DynamicObject dynamic = (DynamicObject)renderable;
        Object active = LayerCapture.field(renderable,dy.class,"ac",qr.class);
        Object previous = LayerCapture.field(renderable,dy.class,"aa",qr.class);
        Map<String,Object> state = controller(active);
        if (dynamic.getAnimFrame() != (int)state.get("frame"))
            throw new IllegalStateException("Native frame getter and verified field decode disagree");
        Animation animation = dynamic.getAnimation();
        if (animation==null || animation.getId()!=(int)state.get("sequence_id"))
            throw new IllegalStateException("Native animation getter and sequence field disagree");
        int lastRaw = raw(renderable,dy.class,"ao");
        int cycleRaw = raw(null,client.class,"cm");
        int cycle = cycleRaw*1612595797;
        if (cycle != layer.capture.game.getGameCycle())
            throw new IllegalStateException("Native source-cycle getter and field disagree");
        int config = object instanceof GameObject ? ((GameObject)object).getConfig()
            : ((DecorativeObject)object).getConfig();
        var tile = object.getWorldLocation();
        return OriginalCapture.map(
            "case_id",layer.id,"observation_phase",phase,
            "source_object_id",object.getId(),"world_tile",new int[]{tile.getX(),tile.getY(),tile.getPlane()},
            "placement_type",config&31,"orientation",(config>>6)&3,"native_config",config,
            "renderable_slot",slot,"native_renderable_type",renderable.getClass().getName(),
            "sequence_id",animation.getId(),"native_frame",dynamic.getAnimFrame(),
            "native_api_anim_cycle",dynamic.getAnimCycle(),
            "native_api_anim_cycle_availability","Unavailable: unchanged DynamicObject.getAnimCycle returns -1.",
            "source_cycle",cycle,"source_cycle_raw",cycleRaw,"source_cycle_decode_multiplier",1612595797,
            "last_update_cycle",lastRaw*1618438999,"last_update_cycle_raw",lastRaw,
            "last_update_decode_multiplier",1618438999,
            "pending_source_cycle_delta",cycle-lastRaw*1618438999,
            "rendering_controller","dy.ac","active_controller",state,"previous_controller",controller(previous),
            "previous_controller_qualification","dy.aa is auxiliary previous-controller state, not the active controller used by dy.ae/rf.",
            "native_world_anchor_x_height_y",new int[]{object.getX(),object.getZ(),object.getY()},
            "observation_effect","Read-only getters/fields; no getModel, animation stepping, phase reset or RNG draw.");
    }

    private void observe(String phase) throws Exception
    {
        Scene scene = layer.world.ae;
        for (int[] target : TARGETS)
        {
            Tile tile = scene.getTiles()[target[3]][target[1]-layer.baseX][target[2]-layer.baseY];
            if (tile==null) throw new IllegalStateException("Requested source tile unavailable");
            int matched=0;
            for (GameObject object : tile.getGameObjects())
            {
                if (object!=null && object.getId()==target[0])
                {
                    observations.add(placement(phase,object,object.getRenderable(),"game-object"));
                    matched++;
                }
            }
            DecorativeObject decoration = tile.getDecorativeObject();
            if (decoration!=null && decoration.getId()==target[0])
            {
                observations.add(placement(phase,decoration,decoration.getRenderable(),"decorative-renderable-1"));
                matched++;
                if (decoration.getRenderable2()!=null)
                    throw new IllegalStateException("Unexpected second animated decoration: preserve it explicitly before proceeding");
            }
            if (matched!=1) throw new IllegalStateException("Missing/ambiguous requested native placement: "+target[0]);
        }
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
                if(value.getAsJsonObject().get("id").getAsString().equals(args[3]))selected=value.getAsJsonObject();
            if(selected==null || !List.of("tutorial-door-closed","tutorial-door-open","tutorial-roofs-hidden").contains(args[3]))
                throw new IllegalArgumentException("Only the three existing admitted Tutorial cases may be replayed");
            try(OriginalCache cache=new OriginalCache(Path.of(args[0])))
            {
                LayerCapture layer=new LayerCapture(new OriginalCapture(cache,Path.of(args[1])),selected);
                SceneryPhaseProbe probe=new SceneryPhaseProbe(layer);
                layer.world();
                probe.observe("after-original-maploader");
                layer.hud();
                if(selected.get("layer").getAsString().equals("door"))layer.door();
                layer.minimap();
                probe.observe("before-original-draw");
                layer.render();
                probe.observe("after-original-draw");
                Files.writeString(Path.of(args[4]),OriginalCapture.JSON.toJson(OriginalCapture.map(
                    "schema_version",1,"case_id",args[3],"classification","controlled offline original-client rendering",
                    "observation_phases",List.of("after-original-maploader","before-original-draw","after-original-draw"),
                    "observations",probe.observations,"seed",0,
                    "source_execution_order","Unchanged LayerCapture.world/hud/[door]/minimap/render; only read-only observations inserted.",
                    "candidate_images_or_frame_guesses_read",false,"animation_state_modified_by_probe",false,
                    "authenticated_source_gameplay",false,"new_presentation_approval",false)));
                System.out.println("SCENERY_PHASE_OBSERVATIONS "+probe.observations.size()+" case="+args[3]);
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
