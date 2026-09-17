import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import com.google.gson.GsonBuilder;

/** Uses the current original runtime's eight-bit compatibility mode only to identify older recordings. */
public final class LegacyCueProbe
{
    public static void main(String[] args) throws Exception
    {
        if (args.length != 3) throw new IllegalArgumentException("LegacyCueProbe verified-old-inputs candidate-inputs output");
        var base = SourceAudio.OfflineArchive.load(Path.of(args[0]), 4);
        var candidates = SourceAudio.OfflineArchive.load(Path.of(args[1]), 4);
        Path output = Path.of(args[2]);
        Files.createDirectories(output);
        var records = new ArrayList<>();
        for (int id : new int[]{710, 711, 713, 2393, 2692, 2693, 2700, 2702, 2725, 2735})
        {
            var archive = Files.exists(Path.of(args[1], "raw/4/" + id + "/0.bin")) ? candidates : base;
            al effect = al.af(archive, id, 0);
            aj raw = effect.ae(true);
            ByteBuffer pcm = ByteBuffer.allocate(raw.af.length * 2).order(ByteOrder.LITTLE_ENDIAN);
            for (short sample : raw.af) pcm.putShort(sample);
            SourceAudio.wav(output.resolve("sfx-" + id + ".wav"), raw.az, 1, pcm.array());
            var record = SourceAudio.rawMetadata(raw);
            record.put("source_id", id);
            record.put("source_native_method", "al.ae(true)");
            record.put("purpose", "Historical recording comparison only, not a replacement of the source16-bit runtime files");
            records.add(record);
        }
        Files.writeString(output.resolve("evidence.json"), new GsonBuilder().setPrettyPrinting().create().toJson(records) + "\n");
        System.out.println("Generated 10 original-runtime compatibility fingerprints; no published files changed.");
    }
}
