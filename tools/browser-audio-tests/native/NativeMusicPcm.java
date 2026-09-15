import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;

final class NativeMusicPcm
{
    static byte[] render(Path inputs, int index, int group, int frames, int volume, ArrayList<Object> reports) throws Exception
    {
        var effects=SourceAudio.OfflineArchive.load(inputs,4);
        var samples=SourceAudio.OfflineArchive.load(inputs,14);
        var patches=SourceAudio.OfflineArchive.load(inputs,15);
        no track=new no(new xy(Files.readAllBytes(inputs.resolve("raw/"+index+"/"+group+"/0.bin"))));
        var device=new SourceAudio.OriginalDevice();
        nu synth=new nu(device.device);
        synth.ap(9,128,(short)-27396);
        NativePolicyProbe.require(synth.ae(track,patches,new at(effects,samples),Integer.MAX_VALUE),"Native patches not loaded");
        for(vq node=synth.al.ab();node!=null;node=synth.al.ag())
            for(Object sample:((nr)node).aa)
                NativePolicyProbe.require(((au)sample).ae(Integer.MAX_VALUE)!=null,"Original native sample not decoded");
        synth.az(volume,0);
        synth.aj(track,false,(byte)2);
        device.device.ae(synth,(byte)0);
        int[] block=new int[1024];
        for(int offset=0;offset<frames;offset+=512)
        {
            device.device.aa(block,512);
            device.write(block,Math.min(512,frames-offset));
        }
        byte[] pcm=device.pcm.toByteArray();
        reports.add(SourceAudio.map("index",index,"group",group,"frames",frames,"volume",volume,
            "pcm_s16le_sha256",SourceAudio.hash(pcm),"native_pre_device_peak",device.peak,
            "native_device_clipped_samples",device.clipped));
        return pcm;
    }

    static void run(Path inputs,Path output) throws Exception
    {
        var manifest=SourceAudio.JSON.fromJson(Files.readString(Path.of("assets/manifests/osrs/audio-runtime.json")),
            com.google.gson.JsonObject.class);
        var reports=new ArrayList<Object>();
        var comparisons=new ArrayList<Object>();
        for(var value:manifest.getAsJsonArray("assets"))
        {
            var asset=value.getAsJsonObject();
            String kind=asset.get("kind").getAsString();
            int group=asset.get("source_group").getAsInt();
            if(!kind.equals("jingle") && !kind.equals("music"))continue;
            int index=kind.equals("jingle")?11:6;
            int frames=asset.getAsJsonObject("signal").get("frames").getAsInt();
            byte[] original=render(inputs,index,group,frames,128,reports);
            NativePolicyProbe.require(SourceAudio.hash(original).equals(
                asset.getAsJsonObject("encoding").get("source_pcm_s16le_sha256").getAsString()),
                "Native control render differs from the unchanged original128 PCM");
            byte[] full=render(inputs,index,group,frames,255,reports);
            int modelClips=0,actualFullScale=0,maxIntegerError=0,extraClips=0;
            double nativeEnergy=0,modelEnergy=0;
            ByteBuffer a=ByteBuffer.wrap(original).order(ByteOrder.LITTLE_ENDIAN);
            ByteBuffer b=ByteBuffer.wrap(full).order(ByteOrder.LITTLE_ENDIAN);
            for(int i=0;i<frames*2;i++)
            {
                double expected=a.getShort()*255.0/128;
                int actual=b.getShort();
                if(expected<-32768 || expected>32767)modelClips++;
                if((expected<-32768 || expected>32767) && actual!=-32768 && actual!=32767)extraClips++;
                if(actual==-32768 || actual==32767)actualFullScale++;
                double limited=Math.max(-32768,Math.min(32767,expected));
                nativeEnergy+=(double)actual*actual;
                modelEnergy+=limited*limited;
                maxIntegerError=Math.max(maxIntegerError,Math.abs(actual-(int)Math.max(-32768,Math.min(32767,expected))));
            }
            comparisons.add(SourceAudio.map("index",index,"group",group,
                "decoded128_times255_over128_clip_samples",modelClips,
                "native255_full_scale_samples",actualFullScale,"additional_clip_samples",extraClips,
                "output_rms_gain_error_db",10*Math.log10(modelEnergy/nativeEnergy),
                "post_quantization_max_integer_difference",maxIntegerError));
            if(group==33)
            {
                Files.createDirectories(output);
                Files.write(output.resolve("jingle33-native255.s16le"),full);
            }
        }
        NativePolicyProbe.cases.add(SourceAudio.map("case","original-current-native-mixer-pcm-control-renders","observed",reports));
        NativePolicyProbe.cases.add(SourceAudio.map("case","immutable128-pcm-native255-mixer-comparison","observed",comparisons));
    }
}
