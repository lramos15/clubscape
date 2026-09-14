import java.io.IOException;
import java.nio.file.Path;
import java.util.Arrays;
import net.runelite.cache.fs.Archive;
import net.runelite.cache.fs.Store;

/** Mutates in-memory copies only; the retrieved cache files are never edited. */
public final class CacheIntegrityTest
{
    public static void main(String[] args) throws Exception
    {
        if (args.length != 1) throw new IllegalArgumentException("Usage: CacheIntegrityTest CACHE");
        try (Store store = new Store(Path.of(args[0]).toFile()))
        {
            store.load();
            Archive archive = store.findIndex(7).getArchive(1570);
            byte[] diskData = store.getStorage().loadArchive(archive);
            byte[] data = Arrays.copyOf(diskData, diskData.length - 2);
            if (archive.getFiles(data).findFile(0).getContents().length == 0)
            {
                throw new AssertionError("Original tree model payload is empty");
            }
            byte[] damaged = data.clone();
            damaged[damaged.length - 1] ^= 1;
            boolean corruptRejected = false;
            try { archive.getFiles(damaged); }
            catch (IOException | RuntimeException expected) { corruptRejected = true; }
            if (!corruptRejected) throw new AssertionError("Corrupted source container was accepted");
            archive.setCrc(archive.getCrc() ^ 1);
            boolean crcRejected = false;
            try { archive.getFiles(data); }
            catch (IOException expected) { crcRejected = true; }
            if (!crcRejected) throw new AssertionError("Incorrect source CRC was accepted");
            System.out.println("Actual original model container passes; corrupted bytes and wrong CRC both rejected.");
        }
    }
}
