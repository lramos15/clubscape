import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.zip.CRC32;
import net.runelite.cache.fs.Archive;
import net.runelite.cache.fs.Index;
import net.runelite.cache.fs.Store;

/** Reuses OriginalCache's verified native JS5 loading path, not a new cache renderer. */
final class VerifiedCache implements AutoCloseable
{
    final Store store;
    private final Map<Integer, vp> archives = new LinkedHashMap<>();
    private final vb queue = new vb();
    private final Evidence evidence;

    VerifiedCache(Path root, Evidence evidence) throws IOException
    {
        this.evidence = evidence;
        store = new Store(root.toFile());
        store.load();
    }

    vp archive(int id) throws Exception
    {
        if (archives.containsKey(id)) return archives.get(id);
        Index index = store.findIndex(id);
        if (index == null) throw new IOException("Missing pinned cache index " + id);
        vp nativeArchive = new vp(null, null, queue, id, false, false, false, false, false);
        nativeArchive.az(store.getStorage().loadArchive(new Archive(new Index(255), id)));
        for (Archive group : index.getArchives())
        {
            byte[] container = store.getStorage().loadArchive(group);
            if (container == null || container.length < 7) throw new IOException("Missing source container");
            int length = ByteBuffer.wrap(container, 1, 4).getInt() + 5 + (container[0] == 0 ? 0 : 4);
            if (length < 5 || container.length != length + 2
                || (ByteBuffer.wrap(container, length, 2).getShort() & 65535) != (group.getRevision() & 65535))
                throw new IOException("Native cache version/container mismatch");
            CRC32 crc = new CRC32();
            crc.update(container, 0, length);
            if ((int) crc.getValue() != group.getCrc())
                throw new IOException("Native cache CRC mismatch");
            nativeArchive.bf[group.getArchiveId()] = Arrays.copyOf(container, length);
        }
        archives.put(id, nativeArchive);
        evidence.record("source_cache_index", "index", id, "groups", index.getArchives().size(),
            "native_decoder", "vp.az / va.bw", "crc_and_disk_versions", "verified");
        return nativeArchive;
    }

    public void close() throws IOException { store.close(); }
}
