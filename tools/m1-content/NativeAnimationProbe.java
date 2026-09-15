import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.TreeMap;
import net.runelite.cache.definitions.loaders.SequenceLoader;

/** Source sequence metadata only; no renderer/avatar export or game loop. */
public final class NativeAnimationProbe
{
    public static void main(String[] args)
    {
        try (OriginalCache cache = new OriginalCache(Path.of(args[0])))
        {
            var archive = cache.store.findIndex(2).getArchive(12);
            var loader = new SequenceLoader().configureForRevision(archive.getRevision());
            var records = new TreeMap<Integer, Object>();
            int[] ids = {386, 390, 392, 393, 395, 396, 397, 400, 401, 402, 422, 423, 424, 426, 428, 429, 440,
                621, 625, 711, 733, 808, 819, 824, 827, 829, 836, 879, 896, 897, 898, 899, 2305,
                4847, 4848, 4849, 4850, 4851, 4852, 4853, 4854, 4855, 4856, 4857, 4858, 12526};
            for (int id : ids)
            {
                byte[] raw = cache.archive(2).loadData(12, id);
                if (raw == null) throw new IllegalStateException("Missing required source sequence " + id);
                var sequence = loader.load(id, raw);
                records.put(id, OriginalCapture.map(
                    "id", id, "sha256", OriginalCapture.hash(raw), "bytes", raw.length,
                    "frame_lengths", sequence.frameLengths,
                    "frame_ids", sequence.frameIDs,
                    "frame_length_sum", sequence.frameLengths == null ? null : Arrays.stream(sequence.frameLengths).sum(),
                    "frame_step", sequence.frameStep, "max_loops", sequence.maxLoops,
                    "left_hand_item", sequence.leftHandItem, "right_hand_item", sequence.rightHandItem,
                    "precedence_animating", sequence.precedenceAnimating,
                    "priority", sequence.priority, "forced_priority", sequence.forcedPriority,
                    "reply_mode", sequence.replyMode, "maya_id", sequence.animMayaID,
                    "maya_start", sequence.animMayaStart, "maya_end", sequence.animMayaEnd));
            }
            Files.writeString(Path.of(args[1]), OriginalCapture.JSON.toJson(OriginalCapture.map(
                "schema_version", 1, "cache_id", 2695, "game_revision", 240,
                "config_archive_revision", archive.getRevision(), "sequences", records,
                "native_indexes", cache.provenance,
                "classification", "CRC/version-checked source metadata; not animation-to-action or server-timing proof")) + "\n");
            System.out.println("ANIMATION_METADATA sequences=" + records.size());
        }
        catch (Throwable error)
        {
            error.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
