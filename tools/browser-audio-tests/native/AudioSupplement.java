import com.google.gson.JsonObject;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import net.runelite.cache.fs.ArchiveFiles;

/** Only archive IO is adapted; MIDI, patches, samples, PcmPlayer and quantization are unchanged native code. */
public final class AudioSupplement
{
    static final int RATE = 22050;
    static final int BLOCK = 512;

    static Object allocate(Class<?> type) throws Exception
    {
        var unsafe = Class.forName("sun.misc.Unsafe");
        var field = unsafe.getDeclaredField("theUnsafe");
        field.setAccessible(true);
        return unsafe.getMethod("allocateInstance",Class.class).invoke(field.get(null),type);
    }

    static void require(boolean test,String message)
    {
        if(!test)throw new IllegalStateException(message);
    }

    static final class Archive extends vp
    {
        BindingCache cache;
        int index;
        Map<Integer,ArchiveFiles> loaded;
        Map<String,Object> reads;

        private Archive() { super(null,null,null,0,false,false,false,false,false); }

        static Archive open(BindingCache cache,int index,Map<String,Object> reads) throws Exception
        {
            Archive archive=(Archive)allocate(Archive.class);
            archive.cache=cache;archive.index=index;archive.reads=reads;archive.loaded=new LinkedHashMap<>();
            var metadata=cache.index(index);
            int[] ids=metadata.getArchives().stream().mapToInt(a->a.getArchiveId()).toArray();
            va base=archive;
            int max=Arrays.stream(ids).max().orElseThrow();
            base.bs=new Object[max+1][];
            base.bf=new Object[max+1];
            base.bg=ids;base.ba=ids.length*2034482581;base.bt=base.ba;
            archive.ay=index*-50391747;
            for(var group:metadata.getArchives())
            {
                int last=Arrays.stream(group.getFileData()).mapToInt(f->f.getId()).max().orElse(0);
                base.bs[group.getArchiveId()]=new Object[last+1];
            }
            return archive;
        }

        byte[] read(int group,int file)
        {
            try
            {
                require(!net.runelite.api.overlay.OverlayIndex.hasOverlay(index,group),"Unexpected original audio overlay");
                ArchiveFiles files=loaded.get(group);
                if(files==null){files=cache.group(index,group);loaded.put(group,files);}
                var entry=files.findFile(file);
                require(entry!=null,"Missing original input "+index+"/"+group+"/"+file);
                byte[] bytes=entry.getContents();
                reads.putIfAbsent(index+"/"+group+"/"+file,SourceAudio.map(
                    "index",index,"group",group,"file",file,"size_bytes",bytes.length,
                    "sha256",SourceAudio.hash(bytes)));
                return bytes;
            }
            catch(Exception error){throw new IllegalStateException("Original read-only archive failed",error);}
        }

        @Override public byte[] bd(int group,int file,int guard){return read(group,file);}
        @Override byte[] bi(int group,int file,int[] keys,int guard)
        {
            require(keys==null,"No encrypted source input is selected");
            return read(group,file);
        }
        @Override public void ds(int group){throw new IllegalStateException("Native network archive request is forbidden");}
    }

    static String key(int index,int group){return (index==6?"music":"jingle")+"-"+group+"-native255";}

    static void allowed(int index,int group)
    {
        require((index==6 && List.of(64,327,163,145).contains(group)) ||
            (index==11 && List.of(40,54,58,64,65).contains(group)),"Outside the exact additive authority");
    }

    static void prepare(BindingCache cache,Path output) throws Exception
    {
        var list=new ArrayList<Object>();
        for(int[] id:new int[][]{{6,64},{6,327},{6,163},{6,145},{11,40},{11,54},{11,58},{11,64},{11,65}})
        {
            var file=cache.group(id[0],id[1]).findFile(0);
            require(file!=null,"Missing original track");
            byte[] encoded=file.getContents();
            no track=new no(new xy(encoded));
            String name=key(id[0],id[1]);
            Files.write(output.resolve(name+".mid"),track.af);
            list.add(SourceAudio.map("index",id[0],"group",id[1],"file",0,
                "source_sha256",SourceAudio.hash(encoded),"midi_sha256",SourceAudio.hash(track.af),
                "midi_file",name+".mid","source_group",cache.groups.get(id[0]+"/"+id[1])));
        }
        Files.writeString(output.resolve("prepared.json"),SourceAudio.JSON.toJson(list)+"\n");
    }

    static void render(BindingCache cache,Path output,JsonObject job) throws Exception
    {
        int index=job.get("index").getAsInt(),group=job.get("group").getAsInt();
        int volume=job.has("native_level")?job.get("native_level").getAsInt():255;
        int end=job.get("expected_engine_end_frame").getAsInt();
        allowed(index,group);
        require(end>0 && end<RATE*600,"Invalid bounded original MIDI clock");
        require(volume>=0 && volume<=255,"Invalid native control level");
        String key=(index==6?"music":"jingle")+"-"+group+"-native"+volume;
        Map<String,Object> reads=new TreeMap<>();
        Archive tracks=Archive.open(cache,index,reads);
        Archive effects=Archive.open(cache,4,reads);
        Archive samples=Archive.open(cache,14,reads);
        Archive patches=Archive.open(cache,15,reads);
        byte[] encoded=tracks.read(group,0);
        no track=new no(new xy(encoded));
        no.li(track);
        var relationships=new ArrayList<Object>();
        Map<String,Integer> sampleReferences=new TreeMap<>();
        for(vq node=track.az.ab();node!=null;node=track.az.ag())
        {
            nj requirement=(nj)node;
            int patchId=(int)requirement.ho;
            nr patch=new nr(patches.read(patchId,0));
            var keys=new ArrayList<Object>();
            for(int note=requirement.az.nextSetBit(0);note>=0;note=requirement.az.nextSetBit(note+1))
            {
                int reference=patch.ao[note];
                require(reference>0,"Required original patch key has no sample");
                int sampleIndex=((reference-1)&1)==0?4:14, sampleGroup=(reference-1)>>2;
                sampleReferences.put(sampleIndex+"/"+sampleGroup,reference);
                keys.add(SourceAudio.map("key",note,"sample_index",sampleIndex,"sample_group",sampleGroup,
                    "encoded_sample_reference",reference,"pitch_and_loop",Short.toUnsignedInt(patch.ab[note]),
                    "exclusive_class",(int)patch.ag[note],"key_volume",(int)patch.as[note],
                    "key_pan",Byte.toUnsignedInt(patch.ac[note])));
            }
            relationships.add(SourceAudio.map("patch",patchId,"patch_volume",patch.af*-127646999,"keys",keys));
        }
        lg.al=RATE*(int)3883844509L;kg.aj=true;
        var device=new SourceAudio.OriginalDevice();
        nu synth=new nu(device.device);
        synth.ap(9,128,(short)-27396);
        require(synth.ac[9]==128 && synth.aq[9]==128 && synth.bx[9]==128,"Native percussion initialization changed");
        at soundCache=new at(effects,samples);
        require(synth.ae(track,patches,soundCache,Integer.MAX_VALUE),"Native instrument load failed");
        for(vq node=synth.al.ab();node!=null;node=synth.al.ag())
            for(Object sample:((nr)node).aa)
                require(((au)sample).ae(Integer.MAX_VALUE)!=null,"Native sample decode failed");
        var decodedSamples=new TreeMap<String,Object>();
        for(var entry:sampleReferences.entrySet())
        {
            int ref=entry.getValue()-1;
            aj raw=(ref&1)==0?soundCache.ae(ref>>2,null,0):soundCache.ab(ref>>2,(byte)1).ae(Integer.MAX_VALUE);
            require(raw!=null && raw.af.length>0,"Missing native sample PCM");
            decodedSamples.put(entry.getKey(),SourceAudio.rawMetadata(raw));
        }
        synth.az(volume,0);
        synth.aj(track,false,(byte)2);
        device.device.ae(synth,(byte)0);
        int[] block=new int[BLOCK*2];
        int frames=end+RATE, endBlock=-1;
        for(int offset=0;offset<frames;offset+=BLOCK)
        {
            device.device.aa(block,BLOCK);
            if(endBlock<0 && !synth.aq((byte)1))endBlock=offset;
            device.write(block,Math.min(BLOCK,frames-offset));
        }
        require(endBlock>=0 && end>endBlock && end<=endBlock+BLOCK,"Native MIDI EOT clock mismatch");
        byte[] pcm=device.pcm.toByteArray();
        SourceAudio.wav(output.resolve(key+".wav"),RATE,2,pcm);
        Map<String,Object> groups=new TreeMap<>();
        for(String input:reads.keySet())
        {
            String[] parts=input.split("/");
            String sourceGroup=parts[0]+"/"+parts[1];
            groups.put(sourceGroup,cache.groups.get(sourceGroup));
        }
        Files.writeString(output.resolve(key+".json"),SourceAudio.JSON.toJson(SourceAudio.map(
            "index",index,"group",group,"file",0,"native_volume",volume,"sample_rate",RATE,"channels",2,
            "frames",frames,"source_engine_end_frame",end,"release_tail_frames",RATE,
            "native_eot_block_start",endBlock,"native_eot_block_end",endBlock+BLOCK,
            "native_midi_loop",false,"native_device_block_frames",BLOCK,
            "native_startup_percussion_channel",9,"native_startup_percussion_bank",128,
            "source_track_sha256",SourceAudio.hash(encoded),"native_midi_sha256",SourceAudio.hash(track.af),
            "pcm_s16le_sha256",SourceAudio.hash(pcm),"pre_device_peak_s24",device.peak,
            "native_device_clipped_samples",device.clipped,"source_inputs",reads,
            "source_groups",groups,"instrument_keys",relationships,"decoded_samples",decodedSamples))+"\n");
    }

    public static void main(String[] args)
    {
        try(BindingCache cache=new BindingCache(Path.of(args[1])))
        {
            Path output=Path.of(args[2]);
            Files.createDirectories(output);
            if(args[0].equals("prepare"))prepare(cache,output);
            else if(args[0].equals("render"))
            {
                var request=SourceAudio.JSON.fromJson(Files.readString(Path.of(args[3])),JsonObject.class);
                for(var job:request.getAsJsonArray("jobs"))render(cache,output,job.getAsJsonObject());
            }
            else throw new IllegalArgumentException("AudioSupplement prepare|render cache output [jobs]");
        }
        catch(Throwable error){error.printStackTrace();System.exit(1);}
        System.exit(0);
    }
}
