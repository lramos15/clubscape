import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;
import net.runelite.cache.definitions.loaders.ScriptLoader;
import net.runelite.cache.definitions.loaders.SequenceLoader;
import net.runelite.cache.definitions.loaders.SpotAnimLoader;
import net.runelite.cache.definitions.loaders.TrackLoader;
import net.runelite.cache.definitions.loaders.sound.SoundEffectLoader;
import net.runelite.cache.fs.ArchiveFiles;
import net.runelite.cache.fs.FSFile;
import net.runelite.cache.fs.Store;

public final class CacheAudioInputs implements AutoCloseable
{
    static final Gson JSON = new GsonBuilder().setPrettyPrinting().serializeNulls().create();
    final Store store;
    final Path output;
    final Map<String, Object> groups = new LinkedHashMap<>();
    final Map<String, ArchiveFiles> loaded = new LinkedHashMap<>();
    final List<Object> outputs = new ArrayList<>();

    CacheAudioInputs(Path cache, Path output) throws Exception
    {
        this.output = output;
        store = new Store(cache.toFile());
        store.load();
    }

    static Map<String, Object> map(Object... values)
    {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < values.length; i += 2)
        {
            result.put((String) values[i], values[i + 1]);
        }
        return result;
    }

    static String hash(byte[] bytes) throws Exception
    {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    ArchiveFiles group(int index, int id) throws Exception
    {
        String key = index + "/" + id;
        if (loaded.containsKey(key))
        {
            return loaded.get(key);
        }
        var archive = store.findIndex(index).getArchive(id);
        if (archive == null)
        {
            throw new IOException("Missing original group " + key);
        }
        byte[] data = store.getStorage().loadArchive(archive);
        if (data == null || data.length < 7)
        {
            throw new IOException("Missing/short original group " + key);
        }
        int compressed = ByteBuffer.wrap(data, 1, 4).getInt();
        long end = 5L + compressed + (data[0] == 0 ? 0 : 4);
        if (compressed < 0 || end + 2 != data.length
            || (ByteBuffer.wrap(data, data.length - 2, 2).getShort() & 65535) != (archive.getRevision() & 65535))
        {
            throw new IOException("Original group length/revision mismatch " + key);
        }
        var crc = new java.util.zip.CRC32();
        crc.update(data, 0, (int) end);
        if (crc.getValue() != Integer.toUnsignedLong(archive.getCrc()))
        {
            throw new IOException("Original group CRC mismatch " + key);
        }
        ArchiveFiles files = archive.getFiles(Arrays.copyOf(data, (int) end));
        loaded.put(key, files);
        groups.put(key, map("index", index, "group", id, "revision", archive.getRevision(),
            "crc32", crc.getValue(), "container_size_bytes", data.length, "container_sha256", hash(data),
            "name_hash", archive.getNameHash(),
            "file_ids", files.getFiles().stream().map(FSFile::getFileId).toList()));
        return files;
    }

    byte[] bytes(int index, int group, int file) throws Exception
    {
        FSFile result = group(index, group).findFile(file);
        if (result == null || result.getContents() == null || result.getContents().length == 0)
        {
            throw new IOException("Missing/empty original file " + index + "/" + group + "/" + file);
        }
        return result.getContents();
    }

    void write(String relative, byte[] bytes) throws Exception
    {
        Path path = output.resolve(relative);
        Files.createDirectories(path.getParent());
        if (Files.exists(path) && !Arrays.equals(Files.readAllBytes(path), bytes))
        {
            throw new IOException("Existing extracted input differs: " + relative);
        }
        Files.write(path, bytes);
        outputs.add(map("path", relative, "size_bytes", bytes.length, "sha256", hash(bytes)));
    }

    void json(String relative, Object value) throws Exception
    {
        write(relative, (JSON.toJson(value) + "\n").getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }

    void extract(Path requestPath) throws Exception
    {
        JsonObject request = JSON.fromJson(Files.readString(requestPath), JsonObject.class);
        TreeSet<Integer> sounds = new TreeSet<>();
        for (var id : request.getAsJsonArray("sound_ids"))
        {
            sounds.add(id.getAsInt());
        }
        TreeSet<Integer> sequences = new TreeSet<>();
        for (var id : request.getAsJsonArray("sequence_ids"))
        {
            sequences.add(id.getAsInt());
        }
        for (var id : request.getAsJsonArray("spot_animation_ids"))
        {
            int spotId = id.getAsInt();
            byte[] raw = bytes(2, 13, spotId);
            var spot = new SpotAnimLoader().load(spotId, raw);
            json("definitions/spot-animation/" + spotId + ".json", spot);
            write("raw/2/13/" + spotId + ".bin", raw);
            if (spot.animationId >= 0)
            {
                sequences.add(spot.animationId);
            }
        }
        var sequenceLoader = new SequenceLoader().configureForRevision(store.findIndex(2).getArchive(12).getRevision());
        for (int sequenceId : sequences)
        {
            byte[] raw = bytes(2, 12, sequenceId);
            var sequence = sequenceLoader.load(sequenceId, raw);
            var definition = JSON.toJsonTree(sequence).getAsJsonObject();
            definition.add("frameSounds", JSON.toJsonTree(sequence.frameSounds.asMap()));
            json("definitions/sequence/" + sequenceId + ".json", definition);
            write("raw/2/12/" + sequenceId + ".bin", raw);
            for (var sound : sequence.frameSounds.values())
            {
                sounds.add(sound.getId());
            }
        }
        for (int id : sounds)
        {
            var files = group(4, id);
            for (FSFile file : files.getFiles())
            {
                write("raw/4/" + id + "/" + file.getFileId() + ".bin", file.getContents());
            }
            json("definitions/sound/" + id + ".json", new SoundEffectLoader().load(bytes(4, id, 0)));
        }
        if (request.has("jingles"))
        {
            for (var value : request.getAsJsonArray("jingles"))
            {
                int id = value.getAsJsonObject().get("id").getAsInt();
                byte[] raw = bytes(11, id, 0);
                byte[] midi = new TrackLoader().load(raw).midi;
                write("raw/11/" + id + "/0.bin", raw);
                write("audio/jingles/" + id + ".mid", midi);
            }
        }
        List<Object> indexes = new ArrayList<>();
        for (int index : new int[]{2, 4, 6, 11, 12, 14, 15})
        {
            var value = store.findIndex(index);
            indexes.add(map("index", index, "revision", value.getRevision(),
                "named", value.isNamed(), "groups", value.getArchives().size()));
        }
        json("supplement.json", map("cache_id", 2695, "build", 240,
            "indexes", indexes, "groups", groups, "outputs", new ArrayList<>(outputs)));
        System.out.println(JSON.toJson(map("sounds", sounds, "indexes", indexes)));
    }

    void layouts(Path preparedPath) throws Exception
    {
        JsonObject prepared = JSON.fromJson(Files.readString(preparedPath), JsonObject.class);
        TreeSet<Integer> sounds = new TreeSet<>();
        for (String path : prepared.getAsJsonObject("inputs").keySet())
        {
            if (path.startsWith("raw/4/"))
            {
                sounds.add(Integer.parseInt(path.split("/")[2]));
            }
        }
        Map<Integer, Object> layouts = new LinkedHashMap<>();
        for (int sound : sounds)
        {
            layouts.put(sound, group(4, sound).getFiles().stream().map(FSFile::getFileId).toList());
        }
        json("layout-check.json", map("cache_id", 2695, "group_file_ids", layouts, "groups", groups));
        System.out.println(JSON.toJson(map("checked_sound_groups", sounds.size(),
            "multiple_file_groups", layouts.entrySet().stream().filter(entry -> ((List<?>) entry.getValue()).size() > 1).toList())));
    }

    @Override public void close() throws IOException
    {
        store.close();
    }

    public static void main(String[] args) throws Exception
    {
        if (args.length == 4 && args[0].equals("check-layouts"))
        {
            try (var importer = new CacheAudioInputs(Path.of(args[1]), Path.of(args[3])))
            {
                importer.layouts(Path.of(args[2]));
            }
            return;
        }
        if (args.length != 3)
        {
            throw new IllegalArgumentException("CacheAudioInputs worktree-cache-copy request.json output-root");
        }
        try (var importer = new CacheAudioInputs(Path.of(args[0]), Path.of(args[2])))
        {
            importer.extract(Path.of(args[1]));
        }
    }
}
