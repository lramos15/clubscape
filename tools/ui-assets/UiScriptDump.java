import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;
import net.runelite.cache.definitions.loaders.ScriptLoader;
import net.runelite.cache.definitions.loaders.EnumLoader;
import net.runelite.cache.definitions.loaders.StructLoader;
import net.runelite.cache.definitions.loaders.GameValLoader;
import net.runelite.cache.definitions.loaders.InterfaceLoader;
import net.runelite.cache.script.Opcodes;

/** Read-only, bounded disassembly of the selected original UI scripts. */
public final class UiScriptDump
{
    static void findVarbitScripts(OriginalCache cache, int varbit) throws Exception
    {
        ScriptLoader loader = new ScriptLoader().configureForRevision(cache.store.findIndex(12).getRevision());
        for (var archive : cache.store.findIndex(12).getArchives())
        {
            byte[] raw = cache.archive(12).loadData(archive.getArchiveId(), 0);
            boolean candidate = false;
            for (int offset = 0; offset + 5 < raw.length; offset++)
                if (raw[offset] == 0 && (raw[offset + 1] == 25 || raw[offset + 1] == 27)
                    && java.nio.ByteBuffer.wrap(raw, offset + 2, 4).getInt() == varbit) candidate = true;
            if (!candidate) continue;
            var script = loader.load(archive.getArchiveId(), raw);
            for (int pc = 0; pc < script.getInstructions().length; pc++)
                if ((script.getInstructions()[pc] == 25 || script.getInstructions()[pc] == 27) && script.getIntOperands()[pc] == varbit)
                {
                    System.out.println("VARBIT_SCRIPT id=" + archive.getArchiveId() + " varbit=" + varbit
                        + " intArgs=" + script.getIntArgCount() + " objectArgs=" + script.getObjArgCount());
                    break;
                }
        }
    }

    static void findWidgetScripts(OriginalCache cache, int group) throws Exception
    {
        ScriptLoader loader = new ScriptLoader().configureForRevision(cache.store.findIndex(12).getRevision());
        for (var archive : cache.store.findIndex(12).getArchives())
        {
            byte[] raw = cache.archive(12).loadData(archive.getArchiveId(), 0);
            boolean candidate = false;
            for (int offset = 0; offset + 5 < raw.length; offset++)
                if (raw[offset] == 0 && raw[offset + 1] == 0
                    && ((raw[offset + 2] & 255) << 8 | raw[offset + 3] & 255) == group)
                {
                    candidate = true;
                    break;
                }
            if (!candidate) continue;
            var script = loader.load(archive.getArchiveId(), raw);
            var references = new ArrayList<Integer>();
            for (int pc = 0; pc < script.getInstructions().length; pc++)
                if (script.getInstructions()[pc] == 0 && script.getIntOperands()[pc] >>> 16 == group)
                    references.add(script.getIntOperands()[pc] & 65535);
            if (!references.isEmpty())
                System.out.println("script=" + archive.getArchiveId() + " group=" + group
                    + " intArgs=" + script.getIntArgCount() + " objectArgs=" + script.getObjArgCount()
                    + " children=" + references);
        }
    }

    static void dump(OriginalCache cache, Path directory, int... ids) throws Exception
    {
        Files.createDirectories(directory);
        Map<Integer, String> names = new LinkedHashMap<>();
        for (var field : Opcodes.class.getFields())
            if (field.getType() == int.class) names.put(field.getInt(null), field.getName());
        ScriptLoader loader = new ScriptLoader().configureForRevision(cache.store.findIndex(12).getRevision());
        for (int id : ids)
        {
            byte[] raw = cache.archive(12).loadData(id, 0);
            var script = loader.load(id, raw);
            var instructions = new ArrayList<Object>();
            for (int pc = 0; pc < script.getInstructions().length; pc++)
            {
                int opcode = script.getInstructions()[pc];
                instructions.add(OriginalCapture.map("pc", pc, "opcode", opcode,
                    "op", names.getOrDefault(opcode, Integer.toString(opcode)),
                    "operand", script.getIntOperands()[pc], "text", script.getStringOperands()[pc]));
            }
            Files.writeString(directory.resolve(id + ".json"), OriginalCapture.JSON.toJson(OriginalCapture.map(
                "id", id, "sha256", OriginalCapture.hash(raw), "intArgs", script.getIntArgCount(),
                "objectArgs", script.getObjArgCount(), "localInts", script.getLocalIntCount(),
                "localObjects", script.getLocalObjCount(), "instructions", instructions,
                "switches", script.getSwitches())));
        }
    }

    public static void main(String[] args) throws Exception
    {
        try (OriginalCache cache = new OriginalCache(Path.of(args[0])))
        {
            for (int i = 2; i < args.length; i++)
                if (args[i].startsWith("labels:"))
                {
                    String term = args[i].substring(7).toLowerCase(java.util.Locale.ROOT);
                    var loader = new GameValLoader();
                    for (int group : new int[]{GameValLoader.INTERFACES, GameValLoader.VARPS, GameValLoader.VARBITS, GameValLoader.VARCS})
                        for (int id : cache.archive(24).getFileIds(group))
                        {
                            var label = loader.load(group, id, cache.archive(24).loadData(group, id));
                            if (label.getName() != null && label.getName().toLowerCase(java.util.Locale.ROOT).contains(term))
                                System.out.println("SOURCE_LABEL group=" + group + " id=" + id + " name=" + label.getName());
                        }
                }
                else if (args[i].startsWith("struct:"))
                {
                    int id = Integer.parseInt(args[i].substring(7));
                    byte[] raw = cache.archive(2).loadData(34, id);
                    Files.createDirectories(Path.of(args[1]));
                    Files.writeString(Path.of(args[1]).resolve("struct-" + id + ".json"), OriginalCapture.JSON.toJson(
                        OriginalCapture.map("sourceSha256", OriginalCapture.hash(raw), "definition", new StructLoader().load(id, raw))));
                }
                else if (args[i].startsWith("enum:"))
                {
                    int id = Integer.parseInt(args[i].substring(5));
                    var value = new EnumLoader().load(id, cache.archive(2).loadData(8, id));
                    Files.writeString(Path.of(args[1]).resolve("enum-" + id + ".json"), OriginalCapture.JSON.toJson(value));
                }
                else if (args[i].startsWith("interface:"))
                {
                    int group = Integer.parseInt(args[i].substring(10));
                    var loader = new InterfaceLoader().configureForRevision(cache.store.findIndex(3).getRevision());
                    var definitions = new ArrayList<Object>();
                    for (int child : cache.archive(3).getFileIds(group))
                    {
                        byte[] raw = cache.archive(3).loadData(group, child);
                        definitions.add(OriginalCapture.map("child", child, "sourceSha256", OriginalCapture.hash(raw),
                            "definition", loader.load(group << 16 | child, raw)));
                    }
                    Files.writeString(Path.of(args[1]).resolve("interface-" + group + ".json"), OriginalCapture.JSON.toJson(definitions));
                    System.out.println("SOURCE_INTERFACE group=" + group + " children=" + definitions.size());
                }
                else if (args[i].startsWith("widget:"))
                    findWidgetScripts(cache, Integer.parseInt(args[i].substring(7)));
                else if (args[i].startsWith("varbit:"))
                    findVarbitScripts(cache, Integer.parseInt(args[i].substring(7)));
                else dump(cache, Path.of(args[1]), Integer.parseInt(args[i]));
        }
        catch (Exception error)
        {
            error.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
