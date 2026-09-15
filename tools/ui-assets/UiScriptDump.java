import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;
import net.runelite.cache.definitions.loaders.ScriptLoader;
import net.runelite.cache.definitions.loaders.EnumLoader;
import net.runelite.cache.script.Opcodes;

/** Read-only, bounded disassembly of the selected original UI scripts. */
public final class UiScriptDump
{
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
                if (args[i].startsWith("enum:"))
                {
                    int id = Integer.parseInt(args[i].substring(5));
                    var value = new EnumLoader().load(id, cache.archive(2).loadData(8, id));
                    Files.writeString(Path.of(args[1]).resolve("enum-" + id + ".json"), OriginalCapture.JSON.toJson(value));
                }
                else if (args[i].startsWith("widget:"))
                    findWidgetScripts(cache, Integer.parseInt(args[i].substring(7)));
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
