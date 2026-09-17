import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Map;
import java.util.Set;
import net.runelite.cache.definitions.ScriptDefinition;

final class NativeScriptPolicy
{
    static final Set<Integer> VARPS = Set.of(18,19,297,3883,3885,3886,5400,3796,5396,5397,5398,5399);
    static final Set<Integer> VARBITS = Set.of(4137,12233,19731,19734,19735,19736,19737,
        20038,20039,20040,20041,20042,20043,20044,20045,20046,20047,20048,20049,20050,20051,20052,20053);

    static void scan(BindingCache cache, Path output) throws Exception
    {
        Map<Integer, ScriptDefinition> all = new LinkedHashMap<>();
        Map<Integer, String> hashes = new LinkedHashMap<>();
        Set<Integer> roots = new LinkedHashSet<>();
        for (var archive : cache.index(12).getArchives())
        {
            int id = archive.getArchiveId();
            byte[] bytes = cache.group(12,id).findFile(0).getContents();
            if (Arrays.equals(bytes, new byte[]{0,9})) continue;
            ScriptDefinition script = BindingExtract.nativeScript(id, bytes);
            all.put(id,script);
            hashes.put(id,SourceAudio.hash(bytes));
            int[] ops = script.getInstructions(), values = script.getIntOperands();
            for (int i = 0; i < ops.length; i++)
                if (((ops[i] == 1 || ops[i] == 2) && VARPS.contains(values[i])) ||
                    ((ops[i] == 25 || ops[i] == 27) && VARBITS.contains(values[i])) ||
                    ops[i] == 3201 || ops[i] == 3221) roots.add(id);
        }
        Set<Integer> selected = new LinkedHashSet<>(roots);
        var queue = new ArrayList<>(roots);
        for (int at = 0; at < queue.size(); at++)
        {
            var script = all.get(queue.get(at));
            int[] ops = script.getInstructions(), values = script.getIntOperands();
            for (int i = 0; i < ops.length; i++)
                if (ops[i] == 40 && selected.add(values[i])) queue.add(values[i]);
        }
        Files.createDirectories(output);
        var summaries = new ArrayList<>();
        for (int id : selected)
        {
            var script = all.get(id);
            if (script == null) throw new IllegalStateException("Missing original script "+id);
            Files.writeString(output.resolve(id+".json"), SourceAudio.JSON.toJson(SourceAudio.map(
                "source_sha256", hashes.get(id), "definition",script)));
            var references = new ArrayList<>();
            int[] ops = script.getInstructions(), values = script.getIntOperands();
            for (int i=0; i<ops.length; i++)
                if (((ops[i] == 1 || ops[i] == 2) && VARPS.contains(values[i])) ||
                    ((ops[i] == 25 || ops[i] == 27) && VARBITS.contains(values[i])) ||
                    ops[i] == 3201 || ops[i] == 3221)
                    references.add(java.util.List.of(i,ops[i],values[i]));
            if (!references.isEmpty()) summaries.add(SourceAudio.map("script",id,
                "int_arguments",script.getIntArgCount(), "object_arguments",script.getObjArgCount(),
                "source_sha256",hashes.get(id),"references",references));
        }
        Map<Integer,String> neededHashes=new LinkedHashMap<>();
        for(int id:selected)neededHashes.put(id,hashes.get(id));
        NativePolicyProbe.cases.add(SourceAudio.map("case","original-music-and-volume-script-reference-census",
            "native_parsed_scripts",all.size(),"selected_script_closure",selected.size(),
            "root_summaries",summaries,"selected_input_hashes",neededHashes,
            "all_native_script_hashes_sha256",SourceAudio.hash(SourceAudio.JSON.toJson(hashes).getBytes(java.nio.charset.StandardCharsets.UTF_8))));
    }
}
