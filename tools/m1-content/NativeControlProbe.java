import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.regex.Pattern;
import net.runelite.cache.definitions.ScriptDefinition;
import net.runelite.cache.definitions.loaders.InterfaceLoader;
import net.runelite.cache.definitions.loaders.EnumLoader;
import net.runelite.cache.definitions.loaders.ScriptLoader;

/** Read-only inspection of source controls; never opens a network transport or exports assets. */
public final class NativeControlProbe
{
    public static void main(String[] args)
    {
        try (OriginalCache cache = new OriginalCache(Path.of(args[0])))
        {
            var scripts = new TreeMap<Integer, ScriptDefinition>();
            var hashes = new TreeMap<Integer, String>();
            var roots = new TreeSet<Integer>();
            var widgets = new TreeMap<Integer, Object>();
            var loader = new ScriptLoader();
            var pattern = Pattern.compile("bank.?all|level.?up|congratulations|Death's Office|Item Retrieval", Pattern.CASE_INSENSITIVE);
            var matches = new ArrayList<Object>();
            var nonScriptRecords = new TreeMap<Integer, String>();
            var sourceScripts = cache.archive(12);
            for (var archive : cache.store.findIndex(12).getArchives())
            {
                int id = archive.getArchiveId();
                byte[] raw = sourceScripts.loadData(id, 0);
                if (raw.length < 18)
                {
                    nonScriptRecords.put(id, java.util.HexFormat.of().formatHex(raw));
                    continue;
                }
                ScriptDefinition script;
                try
                {
                    script = loader.configureForRevision(archive.getRevision()).load(id, raw);
                }
                catch (IllegalArgumentException error)
                {
                    throw new IllegalStateException("Script decode id=" + id + " revision=" + archive.getRevision()
                        + " bytes=" + raw.length + " header="
                        + java.util.HexFormat.of().formatHex(Arrays.copyOf(raw, Math.min(64, raw.length))), error);
                }
                scripts.put(id, script);
                hashes.put(id, OriginalCapture.hash(raw));
                var strings = Arrays.stream(script.getStringOperands())
                    .filter(value -> value != null && pattern.matcher(value).find()).toList();
                if (!strings.isEmpty())
                {
                    matches.add(OriginalCapture.map("id", id, "strings", strings));
                    roots.add(id);
                }
                for (int value : script.getIntOperands())
                    if ((value >>> 16) == 233 || (value >>> 16) == 660) roots.add(id);
            }
            var interfaceLoader = new InterfaceLoader()
                .configureForRevision(cache.store.findIndex(3).getRevision());
            var sourceWidgets = cache.archive(3);
            for (int group : new int[]{12, 233, 602, 660, 669, 672})
            {
                for (int child : sourceWidgets.getFileIds(group))
                {
                    int id = (group << 16) | child;
                    byte[] raw = sourceWidgets.loadData(group, child);
                    var widget = interfaceLoader.load(id, raw);
                    widgets.put(id, OriginalCapture.map("sha256", OriginalCapture.hash(raw), "definition", widget));
                    for (Object[] listener : new Object[][]{
                        widget.onLoadListener, widget.onOpListener, widget.onInvTransmitListener,
                        widget.onVarTransmitListener, widget.onStatTransmitListener})
                        if (listener != null && listener.length > 0 && listener[0] instanceof Integer)
                            roots.add((Integer) listener[0]);
                }
            }
            var selected = new TreeSet<Integer>();
            roots.addAll(Arrays.asList(1989, 3491, 3497, 3498, 3499, 3500, 3502, 3503, 3504));
            var queue = new ArrayDeque<>(roots);
            while (!queue.isEmpty())
            {
                int id = queue.remove();
                if (!selected.add(id)) continue;
                if (selected.size() > 512) throw new IllegalStateException("Source control dependency bound exceeded");
                var script = scripts.get(id);
                if (script == null) throw new IllegalStateException("Missing source script " + id);
                int[] instructions = script.getInstructions(), operands = script.getIntOperands();
                for (int i = 0; i < instructions.length; i++)
                    if (instructions[i] == 40) queue.add(operands[i]);
            }
            var selectedScripts = new LinkedHashMap<Integer, Object>();
            for (int id : selected)
                selectedScripts.put(id, OriginalCapture.map("sha256", hashes.get(id), "definition", scripts.get(id)));
            var enums = new TreeMap<Integer, Object>();
            for (int id : new int[]{1753, 1756, 1757})
            {
                byte[] raw = cache.archive(2).loadData(8, id);
                enums.put(id, OriginalCapture.map("sha256", OriginalCapture.hash(raw),
                    "definition", new EnumLoader().load(id, raw)));
            }
            var output = OriginalCapture.map("schema_version", 1, "cache_id", 2695, "configured_revision", 240,
                "classification", "CRC/version-checked local original cache; no game loop, account or network",
                "script_count", scripts.size(), "matches", matches, "roots", roots,
                "short_non_script_records", nonScriptRecords, "widgets", widgets,
                "scripts", selectedScripts, "enums", enums, "native_indexes", cache.provenance);
            Files.writeString(Path.of(args[1]), OriginalCapture.JSON.toJson(output) + "\n");
            System.out.println("CONTROL_PROBE scripts=" + scripts.size() + " selected=" + selected.size()
                + " widgets=" + widgets.size() + " matches=" + OriginalCapture.JSON.toJson(matches));
        }
        catch (Throwable error)
        {
            error.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
