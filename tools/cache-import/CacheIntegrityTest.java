import java.io.IOException;
import java.nio.file.Path;
import java.util.Arrays;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
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
            System.out.println("Actual full revisions: NPC archive=" + store.findIndex(2).getArchive(9).getRevision()
                + ", item archive=" + store.findIndex(2).getArchive(10).getRevision()
                + ", sequence archive=" + store.findIndex(2).getArchive(12).getRevision()
                + ", interface index=" + store.findIndex(3).getRevision() + "; none is game build 240.");
        }
        Path cache = Path.of(args[0]);
        try (CacheExtractor extractor = new CacheExtractor(cache, cache.getParent().resolve("integrity-no-output"), 240, 2695))
        {
            if (extractor.bytes(2, 9, 4626).length != 108 || extractor.bytes(7, 13897, 0).length == 0)
            {
                throw new AssertionError("Actual Cook definition/model input is missing");
            }
            boolean missingGroup = false, missingFile = false, duplicateId = false;
            try { extractor.bytes(7, Integer.MAX_VALUE, 0); }
            catch (IOException expected) { missingGroup = expected.getMessage().contains("Missing archive"); }
            try { extractor.bytes(2, 9, Integer.MAX_VALUE); }
            catch (IOException expected) { missingFile = expected.getMessage().contains("Missing/empty source file"); }
            JsonObject request = new JsonObject();
            JsonArray ids = new JsonArray();
            ids.add(4626);
            ids.add(4626);
            request.add("npc_ids", ids);
            try { CacheExtractor.requestedIds(request, "npc_ids"); }
            catch (IllegalArgumentException expected) { duplicateId = expected.getMessage().contains("duplicate"); }
            if (!missingGroup || !missingFile || !duplicateId)
            {
                throw new AssertionError("Missing source group/file or duplicate requested ID was accepted");
            }
            System.out.println("Actual Cook 4626/model 13897 pass; missing group/file and duplicate source IDs fail explicitly.");
        }
    }
}
