import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import net.runelite.cache.definitions.ItemDefinition;
import net.runelite.cache.definitions.loaders.ItemLoader;
import net.runelite.cache.fs.ArchiveFiles;
import net.runelite.cache.fs.Container;
import net.runelite.cache.fs.FSFile;
import net.runelite.cache.index.ArchiveData;
import net.runelite.cache.index.IndexData;

/** Decode only the missing named item definitions; never open a writable cache Store. */
public final class DefinitionSupplement {
    private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();

    public static void main(String[] args) throws Exception {
        Path input = Path.of(args[0]);
        IndexData index = new IndexData();
        index.load(Container.decompress(Files.readAllBytes(input.resolve("index2.bin")), null).data);
        ArchiveData archive = Arrays.stream(index.getArchives()).filter(a -> a.getId() == 10)
            .findFirst().orElseThrow();
        byte[] containerBytes = Files.readAllBytes(input.resolve("items.bin"));
        Container container = Container.decompress(
            Arrays.copyOf(containerBytes, containerBytes.length - 2), null);
        if (container.crc != archive.getCrc()) throw new IllegalStateException("Item archive CRC mismatch");
        ArchiveFiles files = new ArchiveFiles();
        for (var file : archive.getFiles()) files.addFile(new FSFile(file.getId()));
        files.loadContents(container.data);
        JsonObject request = GSON.fromJson(Files.readString(input.resolve("request.json")), JsonObject.class);
        Set<String> names = new TreeSet<>();
        request.getAsJsonArray("names").forEach(v -> names.add(v.getAsString()));
        Set<Integer> selected = new TreeSet<>();
        request.getAsJsonArray("ids").forEach(v -> selected.add(v.getAsInt()));
        Map<Integer, ItemDefinition> all = new TreeMap<>();
        ItemLoader loader = new ItemLoader();
        for (FSFile file : files.getFiles()) {
            ItemDefinition item = loader.load(file.getFileId(), file.getContents());
            all.put(item.id, item);
            if (names.contains(item.name)) selected.add(item.id);
        }
        Set<Integer> seeds = new TreeSet<>(selected);
        Map<Integer, Object> result = new TreeMap<>();
        while (!selected.isEmpty()) {
            int id = selected.iterator().next();
            selected.remove(id);
            if (result.containsKey(id)) continue;
            ItemDefinition item = all.get(id);
            if (item == null) throw new IllegalStateException("Missing item " + id);
            Map<String, Object> record = new LinkedHashMap<>();
            byte[] payload = files.findFile(id).getContents();
            record.put("definition", item);
            record.put("payload_size_bytes", payload.length);
            record.put("payload_sha256", HexFormat.of().formatHex(
                MessageDigest.getInstance("SHA-256").digest(payload)));
            record.put("selected_by_name_or_id", seeds.contains(id));
            result.put(id, record);
            for (int related : new int[] {item.notedID, item.notedTemplate, item.boughtId,
                    item.boughtTemplateId, item.placeholderId, item.placeholderTemplateId}) {
                if (related >= 0) selected.add(related);
            }
            if (item.countObj != null) {
                for (int i = 0; i < item.countObj.length; i++) {
                    if (item.countCo[i] > 0) selected.add(item.countObj[i]);
                }
            }
        }
        Map<String, Object> output = new LinkedHashMap<>();
        output.put("archive_revision", archive.getRevision());
        output.put("items", result);
        Files.writeString(input.resolve("decoded.json"), GSON.toJson(output));
        System.out.println("Decoded " + result.size() + " selected/mapping item definitions.");
    }
}
