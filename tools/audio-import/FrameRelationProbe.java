import com.google.gson.GsonBuilder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import net.runelite.cache.definitions.loaders.SequenceLoader;

/** Checks the source frame bytes behind the two retained furnace sequence IDs. */
public final class FrameRelationProbe
{
    public static void main(String[] args) throws Exception
    {
        if (args.length != 2) throw new IllegalArgumentException("FrameRelationProbe readonly-cache output.json");
        try (BindingCache cache = new BindingCache(Path.of(args[0])))
        {
            var group = cache.group(2, 12);
            var loader = new SequenceLoader().configureForRevision(cache.index(2).getArchive(12).getRevision());
            var a = loader.load(899, group.findFile(899).getContents());
            var b = loader.load(3243, group.findFile(3243).getContents());
            var pairs = new ArrayList<>();
            boolean allSame = a.frameIDs.length == b.frameIDs.length;
            for (int i = 0; i < Math.min(a.frameIDs.length, b.frameIDs.length); i++)
            {
                int left = a.frameIDs[i], right = b.frameIDs[i];
                byte[] x = cache.group(0, left >>> 16).findFile(left & 65535).getContents();
                byte[] y = cache.group(0, right >>> 16).findFile(right & 65535).getContents();
                boolean same = Arrays.equals(x, y);
                allSame &= same;
                pairs.add(BindingCache.map("frame_index", i, "sequence899_frame_id", left,
                    "sequence3243_frame_id", right, "left_sha256", BindingCache.hash(x),
                    "right_sha256", BindingCache.hash(y), "raw_frames_identical", same));
            }
            var result = BindingCache.map("sequence_ids", new int[]{899, 3243},
                "frame_lengths_identical", Arrays.equals(a.frameLengths, b.frameLengths),
                "all_raw_frame_payloads_identical", allSame, "frame_pairs", pairs,
                "source_groups", cache.groups);
            Path output = Path.of(args[1]);
            Files.createDirectories(output.getParent());
            Files.writeString(output, new GsonBuilder().setPrettyPrinting().create().toJson(result) + "\n");
            System.out.println("Furnace sequence raw-frame identity: " + allSame);
        }
    }
}
