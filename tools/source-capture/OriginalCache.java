import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import java.util.zip.CRC32;
import net.runelite.cache.fs.Archive;
import net.runelite.cache.fs.Index;
import net.runelite.cache.fs.Store;

/** Original JS5 archive objects backed only by hash-verified local cache containers. */
public final class OriginalCache implements AutoCloseable
{
    final Store store;
    final Map<Integer, vp> archives = new HashMap<>();
    final java.util.List<Object> provenance = new java.util.ArrayList<>();
    final vb offlineQueue = new vb();

    OriginalCache(Path directory) throws IOException
    {
        store = new Store(directory.toFile());
        store.load();
    }

    vp archive(int id) throws Exception
    {
        if (archives.containsKey(id)) return archives.get(id);
        Index index = store.findIndex(id);
        if (index == null) throw new IOException("Missing source index " + id);
        vp original = new vp(null, null, offlineQueue, id, false, false, false, false, false);
        byte[] encodedIndex = store.getStorage().loadArchive(new Archive(new Index(255), id));
        original.az(encodedIndex);
        for (Archive group : index.getArchives())
        {
            byte[] container = store.getStorage().loadArchive(group);
            if (container == null || container.length < 7) throw new IOException("Missing source group");
            int length = ByteBuffer.wrap(container, 1, 4).getInt() + 5 + (container[0] == 0 ? 0 : 4);
            if (container.length != length + 2
                || (ByteBuffer.wrap(container, length, 2).getShort() & 65535) != (group.getRevision() & 65535))
            {
                throw new IOException("Source container/version mismatch: " + id + "/" + group.getArchiveId());
            }
            CRC32 crc = new CRC32();
            crc.update(container, 0, length);
            if ((int) crc.getValue() != group.getCrc()) throw new IOException("Source CRC mismatch");
            original.bf[group.getArchiveId()] = Arrays.copyOf(container, length);
        }
        archives.put(id, original);
        provenance.add(OriginalCapture.map("index", id, "groups", index.getArchives().size(),
            "index_revision", index.getRevision(), "index_container_sha256", OriginalCapture.hash(encodedIndex),
            "native_index_decoder", "vp.az -> va.bw(byte[],1479807699)",
            "container_crc_and_version_checks", "passed",
            "native_group_decompression", "Original va/vp loadData; no network pump"));
        System.out.println("ORIGINAL_JS5 index=" + id + " groups=" + index.getArchives().size());
        return original;
    }

    public void close() throws IOException { store.close(); }
}
