import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;

final class NativeObjects
{
    static Map<String,Object> record(om object, byte[] source) throws Exception
    {
        wj sound=object.df;
        ww random=object.ds;
        return SourceAudio.map("id",object.getId(),"sizeX",object.getSizeX(),"sizeY",object.getSizeY(),
            "varbit",object.getVarbitId(),"varp",object.getVarPlayerId(),"transforms",object.getImpostorIds(),
            "sound",sound==null?null:SourceAudio.map("id",sound.az * -1727985133,
                "range",sound.af * 1535961601,"retain",sound.ae * -2063946921,
                "visibility",sound.ag.ab * -762797223,"distanceCurve",sound.ab.ac(0).az((byte)0),
                "fadeInCurve",sound.ab.ab((byte)0).az((byte)0),"fadeInMs",wd.cv(sound.ab,0),
                "fadeOutCurve",sound.ab.as(0).az((byte)0),"fadeOutMs",sound.ab.ax((byte)0)),
            "random",random==null?null:SourceAudio.map("ids",random.az,
                "minCycles",random.af * 1861224747,"maxCycles",random.ae * -1823764067),
            "source_sha256",SourceAudio.hash(source));
    }

    static void run(Path cachePath) throws Exception
    {
        try(BindingCache cache=new BindingCache(cachePath))
        {
            NativeArchive configs=NativeArchive.open(cache,2);
            ak.cq=configs;pt.ae=configs;lb.af=new int[65536];
            var map=SourceAudio.JSON.fromJson(Files.readString(Path.of("research/audio-source/source-map.json")),
                com.google.gson.JsonObject.class);
            var queue=new ArrayList<Integer>();
            var seen=new LinkedHashSet<Integer>();
            for(var value:map.getAsJsonArray("ambient_objects"))
            {
                int id=value.getAsJsonObject().get("object_id").getAsInt();
                if(seen.add(id))queue.add(id);
            }
            var definitions=new ArrayList<Object>();
            var variableDefinitions=new LinkedHashMap<Integer,Object>();
            var morphCases=new ArrayList<Object>();
            for(int at=0;at<queue.size();at++)
            {
                int id=queue.get(at);
                om object=(om)NativePolicyProbe.instance.getObjectDefinition(id);
                definitions.add(record(object,configs.read(6,id)));
                int[] transforms=object.getImpostorIds();
                if(transforms==null)continue;
                for(int target:transforms)if(target>=0 && seen.add(target))queue.add(target);
                int varp=object.getVarPlayerId(),shift=0,width=32;
                if(object.getVarbitId()>=0)
                {
                    byte[] data=configs.read(14,object.getVarbitId());
                    pt bit=new pt(new xy(data));
                    varp=bit.getIndex();shift=bit.getLeastSignificantBit();
                    width=bit.getMostSignificantBit()-shift+1;
                    variableDefinitions.put(object.getVarbitId(),SourceAudio.map("id",object.getVarbitId(),
                        "varp",varp,"lsb",shift,"msb",bit.getMostSignificantBit(),"source_sha256",SourceAudio.hash(data)));
                }
                NativePolicyProbe.require(varp>=0,"Morph has no bound varp or varbit");
                for(int requested:new int[]{0,1,transforms.length-2,transforms.length-1,255})
                {
                    java.util.Arrays.fill(lb.af,0);
                    int mask=width==32?-1:(1<<width)-1;
                    int selector=requested & mask;
                    lb.af[varp]=selector<<shift;
                    om selected=om.dl(object,0);
                    int actual=selected==null?-1:selected.getId();
                    int expected=selector>=0 && selector<transforms.length-1
                        ?transforms[selector]:transforms[transforms.length-1];
                    NativePolicyProbe.require(actual==expected,"Original morph fallback/selector mismatch");
                    morphCases.add(SourceAudio.map("object",id,"requested",requested,"varp",varp,
                        "varpValue",lb.af[varp],"decoded_selector",selector,"native_selected",actual));
                }
            }
            NativePolicyProbe.cases.add(SourceAudio.map("case","native-m1-object-audio-definitions","definitions",definitions,
                "varbits",variableDefinitions.values()));
            NativePolicyProbe.cases.add(SourceAudio.map("case","actual-om-dl-morph-selection","observed",morphCases));
        }
    }
}
