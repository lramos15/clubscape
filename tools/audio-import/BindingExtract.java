import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import java.nio.file.Files;
import java.nio.file.Path;
import java.math.BigInteger;
import java.util.Arrays;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import net.runelite.cache.definitions.ScriptDefinition;
import net.runelite.cache.definitions.loaders.InterfaceLoader;
import net.runelite.cache.definitions.loaders.SequenceLoader;
import net.runelite.cache.definitions.loaders.sound.SoundEffectLoader;

public final class BindingExtract
{
    static final Gson JSON = new GsonBuilder().setPrettyPrinting().serializeNulls().create();

    static void write(Path root, String path, Object value) throws Exception
    {
        Path target = root.resolve(path);
        Files.createDirectories(target.getParent());
        Files.writeString(target, JSON.toJson(value) + "\n");
    }

    static int decoded(int value, int encoder)
    {
        int inverse = BigInteger.valueOf(Integer.toUnsignedLong(encoder)).modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
        return value * inverse;
    }

    @SuppressWarnings("unchecked")
    static ScriptDefinition nativeScript(int id, byte[] bytes)
    {
        bl original = lw.ab(bytes, 301197489);
        ScriptDefinition result = new ScriptDefinition();
        result.setId(id);
        result.setInstructions(original.getInstructions());
        result.setIntOperands(original.getIntOperands());
        result.setStringOperands(original.as);
        result.setLongOperands(original.ax);
        result.setLocalIntCount(decoded(original.ac, -420896219));
        result.setLocalObjCount(decoded(original.ao, -431593185));
        result.setLocalLongCount(decoded(original.aa, 2139455799));
        result.setIntArgCount(decoded(original.al, 1424730171));
        result.setObjArgCount(decoded(original.aj, 712179175));
        result.setLongArgCount(decoded(original.ay, 1116857197));
        if (original.af != null)
        {
            Map<Integer, Integer>[] switches = (Map<Integer, Integer>[]) new Map<?, ?>[original.af.length];
            for (int i = 0; i < switches.length; i++)
            {
                switches[i] = new LinkedHashMap<>();
                for (Object node : original.af[i])
                {
                    vg entry = (vg) node;
                    switches[i].put((int) entry.getHash(), entry.getValue());
                }
            }
            result.setSwitches(switches);
        }
        return result;
    }

    static void extract(String[] args) throws Exception
    {
        if (args.length != 3) throw new IllegalArgumentException("BindingExtract readonly-cache request.json output");
        Path output = Path.of(args[2]);
        JsonObject request = JSON.fromJson(Files.readString(Path.of(args[1])), JsonObject.class);
        try (BindingCache cache = new BindingCache(Path.of(args[0])))
        {
            Map<Integer, Object> sequences = new LinkedHashMap<>();
            var sequenceFiles = cache.group(2, 12);
            var sequenceLoader = new SequenceLoader().configureForRevision(cache.index(2).getArchive(12).getRevision());
            for (var id : request.getAsJsonArray("sequence_ids"))
            {
                var file = sequenceFiles.findFile(id.getAsInt());
                if (file == null) throw new IllegalArgumentException("Missing requested source sequence");
                var definition = sequenceLoader.load(id.getAsInt(), file.getContents());
                var value = JSON.toJsonTree(definition).getAsJsonObject();
                value.add("frameSounds", JSON.toJsonTree(definition.frameSounds.asMap()));
                sequences.put(id.getAsInt(), BindingCache.map("source_sha256", BindingCache.hash(file.getContents()), "definition", value));
            }
            write(output, "sequences.json", sequences);
            TreeSet<Integer> roots = new TreeSet<>();
            for (var id : request.getAsJsonArray("script_roots")) roots.add(id.getAsInt());
            List<Object> callbacks = new ArrayList<>();
            var interfaceLoader = new InterfaceLoader().configureForRevision(cache.index(3).getRevision());
            for (var id : request.getAsJsonArray("interface_groups"))
            {
                int group = id.getAsInt();
                for (var file : cache.group(3, group).getFiles())
                {
                    int widgetId = group << 16 | file.getFileId();
                    var widget = JSON.toJsonTree(interfaceLoader.load(widgetId, file.getContents())).getAsJsonObject();
                    for (var entry : widget.entrySet())
                    {
                        if (!entry.getKey().endsWith("Listener") || !entry.getValue().isJsonArray()) continue;
                        var listener = entry.getValue().getAsJsonArray();
                        if (listener.size() == 0) continue;
                        roots.add(listener.get(0).getAsInt());
                        callbacks.add(BindingCache.map("widget_id", widgetId, "listener", entry.getKey(),
                            "arguments", listener, "source_sha256", BindingCache.hash(file.getContents())));
                    }
                }

            }
            write(output, "widget-callbacks.json", callbacks);
            Map<Integer, Object> sounds = new LinkedHashMap<>();
            for (var id : request.getAsJsonArray("candidate_sound_ids"))
            {
                int soundId = id.getAsInt();
                var archive = cache.group(4, soundId);
                var file = archive.findFile(0);
                if (archive.getFiles().size() != 1 || file == null)
                    throw new IllegalArgumentException("Candidate has an unhandled source file layout: " + soundId);
                byte[] bytes = file.getContents();
                Path target = output.resolve("candidate-inputs/raw/4/" + soundId + "/0.bin");
                Files.createDirectories(target.getParent());
                Files.write(target, bytes);
                sounds.put(soundId, BindingCache.map("source_sha256", BindingCache.hash(bytes),
                    "size_bytes", bytes.length, "definition", new SoundEffectLoader().load(bytes)));
            }
            write(output, "candidate-sounds.json", sounds);
            var scriptIndex = cache.index(12);
            Map<Integer, ScriptDefinition> scripts = new LinkedHashMap<>();
            Map<Integer, String> scriptHashes = new LinkedHashMap<>();
            List<Object> nonScripts = new ArrayList<>();
            TreeSet<Integer> audioScripts = new TreeSet<>();
            List<Object> audioCalls = new ArrayList<>();
            for (var archive : scriptIndex.getArchives())
            {
                int id = archive.getArchiveId();
                var file = cache.group(12, id).findFile(0);
                if (file == null) throw new IllegalArgumentException("Script group has no file0: " + id);
                ScriptDefinition script;
                try
                {
                    script = nativeScript(id, file.getContents());
                }
                catch (RuntimeException exception)
                {
                    if (Arrays.equals(file.getContents(), new byte[]{0, 9}))
                    {
                        nonScripts.add(BindingCache.map("script_id", id, "source_sha256", BindingCache.hash(file.getContents()),
                            "payload_hex", "0009", "native_decoder_exception", exception.getClass().getName()));
                        continue;
                    }
                    Files.write(output.resolve("failed-script-" + id + ".bin"), file.getContents());
                    throw new IllegalArgumentException("Source script decode failed: id=" + id + ", group_revision="
                        + archive.getRevision() + ", bytes=" + file.getContents().length, exception);
                }
                scripts.put(id, script);
                scriptHashes.put(id, BindingCache.hash(file.getContents()));
                int[] instructions = script.getInstructions(), values = script.getIntOperands();
                for (int pc = 0; pc < instructions.length; pc++)
                {
                    if (instructions[pc] < 3200 || instructions[pc] > 3229) continue;
                    audioScripts.add(id);
                    List<Object> context = new ArrayList<>();
                    for (int j = Math.max(0, pc - 8); j <= Math.min(instructions.length - 1, pc + 2); j++)
                        context.add(List.of(j, instructions[j], values[j]));
                    audioCalls.add(BindingCache.map("script_id", id, "name_hash", archive.getNameHash(),
                        "pc", pc, "opcode", instructions[pc], "context", context));
                }
            }
            TreeSet<Integer> reachable = new TreeSet<>();
            ArrayDeque<Integer> queue = new ArrayDeque<>(roots);
            while (!queue.isEmpty())
            {
                int id = queue.removeFirst();
                if (!reachable.add(id)) continue;
                ScriptDefinition script = scripts.get(id);
                if (script == null) throw new IllegalArgumentException("Missing callback script " + id);
                int[] instructions = script.getInstructions(), values = script.getIntOperands();
                for (int pc = 0; pc < instructions.length; pc++)
                    if (instructions[pc] == 40) queue.add(values[pc]);
            }
            TreeSet<Integer> selected = new TreeSet<>(reachable);
            selected.addAll(audioScripts);
            for (int id : selected)
                write(output, "scripts/" + id + ".json", BindingCache.map("source_sha256", scriptHashes.get(id),
                    "name_hash", scriptIndex.getArchive(id).getNameHash(), "definition", scripts.get(id)));
            write(output, "script-scan.json", BindingCache.map("decoded_scripts", scripts.size(),
                "native_decoder", "injected1.12.38 lw.ab(byte[],301197489)",
                "non_script_payloads_rejected_by_native_decoder", nonScripts,
                "script_index_revision", scriptIndex.getRevision(), "root_scripts", roots, "reachable_scripts", reachable,
                "audio_scripts", audioScripts, "audio_calls", audioCalls, "script_sha256", scriptHashes));
            write(output, "groups.json", cache.groups);
            System.out.println(JSON.toJson(BindingCache.map("decoded_scripts", scripts.size(), "audio_scripts", audioScripts,
                "callback_roots", roots, "callback_closure_size", reachable.size())));
        }
    }

    public static void main(String[] args)
    {
        try
        {
            extract(args);
        }
        catch (Exception exception)
        {
            exception.printStackTrace();
            System.exit(1);
        }
        // Original script initialization creates an idle executor; this isolated probe owns the JVM.
        System.exit(0);
    }
}
