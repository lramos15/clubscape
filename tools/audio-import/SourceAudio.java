import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.lang.reflect.Proxy;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import javax.sound.midi.MidiSystem;
import javax.sound.sampled.SourceDataLine;

public final class SourceAudio
{
    static final Gson JSON = new GsonBuilder().setPrettyPrinting().serializeNulls().create();
    static final int RATE = 22050;
    static final int BLOCK = 512;

    static Map<String, Object> map(Object... values)
    {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < values.length; i += 2)
        {
            result.put((String) values[i], values[i + 1]);
        }
        return result;
    }

    static String hash(byte[] data) throws Exception
    {
        return java.util.HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
    }

    static final class OfflineArchive extends vp
    {
        Path root;
        int index;
        Map<String, Object> reads;

        private OfflineArchive()
        {
            super(null, null, null, 0, false, false, false, false, false);
        }

        static OfflineArchive load(Path root, int index) throws Exception
        {
            Class<?> unsafeClass = Class.forName("sun.misc.Unsafe");
            var field = unsafeClass.getDeclaredField("theUnsafe");
            field.setAccessible(true);
            Object unsafe = field.get(null);
            // The real Archive constructor registers network/disk requests. Never run it.
            OfflineArchive archive = (OfflineArchive) unsafeClass.getMethod("allocateInstance", Class.class)
                .invoke(unsafe, OfflineArchive.class);
            archive.root = root;
            archive.index = index;
            archive.reads = new LinkedHashMap<>();
            List<Path> files;
            try (var stream = Files.walk(root.resolve("raw/" + index)))
            {
                files = stream.filter(p -> p.toString().endsWith(".bin")).sorted().toList();
            }
            int max = files.stream().mapToInt(p -> Integer.parseInt(p.getParent().getFileName().toString()))
                .max().orElseThrow();
            va base = archive;
            base.bs = new Object[max + 1][];
            base.bf = new Object[max + 1];
            base.bg = files.stream().mapToInt(p -> Integer.parseInt(p.getParent().getFileName().toString()))
                .distinct().toArray();
            base.ba = base.bg.length * 2034482581;
            base.bt = base.ba;
            archive.ay = index * -50391747;
            for (Path path : files)
            {
                int group = Integer.parseInt(path.getParent().getFileName().toString());
                int file = Integer.parseInt(path.getFileName().toString().replace(".bin", ""));
                Object[] groupFiles = base.bs[group];
                if (groupFiles == null || groupFiles.length <= file)
                {
                    groupFiles = groupFiles == null ? new Object[file + 1] : Arrays.copyOf(groupFiles, file + 1);
                    base.bs[group] = groupFiles;
                }
                groupFiles[file] = Files.readAllBytes(path);
            }
            if (net.runelite.api.overlay.OverlayIndex.hasOverlay(index, 0))
            {
                throw new IllegalStateException("Unexpected original audio overlay");
            }
            return archive;
        }

        byte[] read(int group, int file)
        {
            try
            {
                va base = this;
                if (group < 0 || group >= base.bs.length || base.bs[group] == null
                    || file < 0 || file >= base.bs[group].length || base.bs[group][file] == null)
                {
                    throw new IllegalStateException("Missing original audio input raw/" + index + "/" + group + "/" + file + ".bin");
                }
                byte[] bytes = (byte[]) base.bs[group][file];
                if (net.runelite.api.overlay.OverlayIndex.hasOverlay(index, group))
                {
                    throw new IllegalStateException("Audio overlays are not source cache bytes");
                }
                String key = index + "/" + group + "/" + file;
                reads.putIfAbsent(key, map("index", index, "group", group, "file", file,
                    "size_bytes", bytes.length, "sha256", hash(bytes)));
                return bytes;
            }
            catch (RuntimeException e)
            {
                throw e;
            }
            catch (Exception e)
            {
                throw new IllegalStateException(e);
            }
        }

        @Override byte[] bi(int group, int file, int[] keys, int unused)
        {
            if (keys != null)
            {
                throw new IllegalArgumentException("Encrypted audio inputs are not selected");
            }
            return read(group, file);
        }

        @Override public byte[] bd(int group, int file, int unused)
        {
            return read(group, file);
        }

        @Override public void ds(int group)
        {
            throw new IllegalStateException("Offline archive cannot load an absent group: " + index + "/" + group);
        }
    }

    static Map<String, Object> rawMetadata(aj raw) throws Exception
    {
        ByteBuffer bytes = ByteBuffer.allocate(raw.af.length * 2).order(ByteOrder.LITTLE_ENDIAN);
        for (short value : raw.af)
        {
            bytes.putShort(value);
        }
        return map("sample_rate", raw.az, "frames", raw.af.length,
            "loop_start_frame", raw.ae, "loop_end_frame", raw.ab,
            "ping_pong_loop", raw.ag, "source_flag_as", raw.as,
            "pcm_s16le_sha256", hash(bytes.array()));
    }

    static void wav(Path file, int rate, int channels, byte[] pcm) throws IOException
    {
        ByteBuffer header = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN);
        header.put("RIFF".getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        header.putInt(36 + pcm.length);
        header.put("WAVEfmt ".getBytes(java.nio.charset.StandardCharsets.US_ASCII));
        header.putInt(16).putShort((short) 1).putShort((short) channels);
        header.putInt(rate).putInt(rate * channels * 2).putShort((short) (channels * 2)).putShort((short) 16);
        header.put("data".getBytes(java.nio.charset.StandardCharsets.US_ASCII)).putInt(pcm.length);
        Files.createDirectories(file.getParent());
        try (var out = Files.newOutputStream(file))
        {
            out.write(header.array());
            out.write(pcm);
        }
    }

    static final class OriginalDevice
    {
        final tv device = new tv();
        final ByteArrayOutputStream pcm = new ByteArrayOutputStream();
        byte[] captured;
        long peak;
        long clipped;

        OriginalDevice()
        {
            device.bj = new byte[BLOCK * 2 * 2];
            device.af = (SourceDataLine) Proxy.newProxyInstance(
                SourceDataLine.class.getClassLoader(), new Class<?>[]{SourceDataLine.class},
                (proxy, method, args) ->
                {
                    if (!method.getName().equals("write"))
                    {
                        throw new IllegalStateException("Audio hardware operation forbidden: " + method.getName());
                    }
                    byte[] bytes = (byte[]) args[0];
                    int start = (int) args[1], length = (int) args[2];
                    captured = Arrays.copyOfRange(bytes, start, start + length);
                    return length;
                });
        }

        void write(int[] block, int frames)
        {
            device.ai = block;
            captured = null;
            // Execute the original PCM device's quantization, but capture its bytes instead of opening a device.
            device.aq();
            if (captured == null || captured.length != BLOCK * 4)
            {
                throw new IllegalStateException("Original device did not write its complete stereo block");
            }
            for (int i = 0; i < frames * 2; i++)
            {
                int value = block[i];
                peak = Math.max(peak, Math.abs((long) value));
                if (value < -8388608 || value > 8388607)
                {
                    clipped++;
                }
                short reference = (short) (Math.max(-8388608, Math.min(8388607, value)) >> 8);
                short actual = (short) ((captured[i * 2] & 255) | ((captured[i * 2 + 1] & 255) << 8));
                if (reference != actual)
                {
                    throw new IllegalStateException("Original device quantization no longer matches the pinned adapter");
                }
            }
            pcm.write(captured, 0, frames * 4);
        }
    }

    static void music(Path root, Path output, JsonObject job) throws Exception
    {
        int id = job.get("group").getAsInt();
        int index = job.get("index").getAsInt();
        String assetKey = (index == 6 ? "music-" : "jingle-") + id;
        int expectedEnd = job.get("expected_engine_end_frame").getAsInt();
        int preview = job.has("preview_frames") ? job.get("preview_frames").getAsInt() : 0;
        if ((index != 6 && index != 11) || expectedEnd <= 0 || expectedEnd > RATE * 600
            || preview < 0 || preview > expectedEnd)
        {
            throw new IllegalArgumentException("Invalid bounded music conversion job");
        }
        OfflineArchive effects = OfflineArchive.load(root, 4);
        OfflineArchive samples = OfflineArchive.load(root, 14);
        OfflineArchive patches = OfflineArchive.load(root, 15);
        byte[] source = Files.readAllBytes(root.resolve("raw/" + index + "/" + id + "/0.bin"));
        no track = new no(new xy(source));
        byte[] expectedMidi = Files.readAllBytes(root.resolve("audio/"
            + (index == 6 ? "music/" : "jingles/") + id + ".mid"));
        Files.write(output.resolve(assetKey + ".runtime.mid"), track.af);
        var sequence = MidiSystem.getSequence(new ByteArrayInputStream(expectedMidi));
        no.li(track);
        List<Object> patchNotes = new ArrayList<>();
        TreeMapSamples decoded = new TreeMapSamples();
        for (vq node = track.az.ab(); node != null; node = track.az.ag())
        {
            nj required = (nj) node;
            int patchId = (int) required.ho;
            nr patch = new nr(patches.read(patchId, 0));
            List<Object> notes = new ArrayList<>();
            for (int key = required.az.nextSetBit(0); key >= 0; key = required.az.nextSetBit(key + 1))
            {
                int ref = patch.ao[key];
                if (ref <= 0)
                {
                    throw new IllegalStateException("A required original instrument key has no sample: " + patchId + "/" + key);
                }
                notes.add(map("key", key, "encoded_sample_reference", ref,
                    "sample_index", ((ref - 1) & 1) == 0 ? 4 : 14, "sample_group", (ref - 1) >> 2,
                    "pitch_and_loop", Short.toUnsignedInt(patch.ab[key]),
                    "native_loop_enabled", patch.ab[key] < 0,
                    "exclusive_class", (int) patch.ag[key],
                    "key_volume", (int) patch.as[key],
                    "key_pan", Byte.toUnsignedInt(patch.ac[key])));
                decoded.references.put(((ref - 1) & 1) == 0 ? "4/" + ((ref - 1) >> 2) : "14/" + ((ref - 1) >> 2), ref);
            }
            patchNotes.add(map("patch_id", patchId, "patch_volume", patch.af * -127646999, "notes", notes));
        }
        lg.al = RATE * (int) 3883844509L;
        kg.aj = true;
        OriginalDevice device = new OriginalDevice();
        nu synth = new nu(device.device);
        synth.ap(9, 128, (short) -27396);
        if (synth.ac[9] != 128 || synth.aq[9] != 128 || synth.bx[9] != 128)
        {
            throw new IllegalStateException("Original startup percussion-channel initialization failed");
        }
        at cache = new at(effects, samples);
        if (!synth.ae(track, patches, cache, Integer.MAX_VALUE))
        {
            throw new IllegalStateException("Original instrument dependencies did not load");
        }
        for (vq node = synth.al.ab(); node != null; node = synth.al.ag())
        {
            nr patch = (nr) node;
            for (Object value : patch.aa)
            {
                aj raw = ((au) value).ae(Integer.MAX_VALUE);
                if (raw == null || raw.af.length == 0)
                {
                    throw new IllegalStateException("Original instrument sample decode failed");
                }
            }
        }
        for (var entry : decoded.references.entrySet())
        {
            int ref = entry.getValue() - 1;
            aj raw = (ref & 1) == 0
                ? cache.ae(ref >> 2, null, 0)
                : cache.ab(ref >> 2, (byte) 1).ae(Integer.MAX_VALUE);
            if (raw == null || raw.af.length == 0)
            {
                throw new IllegalStateException("Missing decoded original sample " + entry.getKey());
            }
            decoded.samples.put(entry.getKey(), rawMetadata(raw));
        }
        synth.az(128, 0);
        synth.aj(track, false, (byte) 2);
        device.device.ae(synth, (byte) 0);
        int[] block = new int[BLOCK * 2];
        int frames = 0;
        int endBlock = -1;
        int limit = preview > 0 ? preview : expectedEnd + RATE;
        while (frames < limit)
        {
            int count = Math.min(BLOCK, limit - frames);
            // Keep the actual player's512-frame scheduling/voice budget as well as its MIDI mixer.
            device.device.aa(block, BLOCK);
            if (endBlock < 0 && !synth.aq((byte) 1))
            {
                endBlock = frames;
            }
            device.write(block, count);
            frames += count;
        }
        if (preview == 0 && (endBlock < 0 || expectedEnd <= endBlock || expectedEnd > endBlock + BLOCK))
        {
            throw new IllegalStateException("Original MIDI EOT block mismatch for " + assetKey + ": "
                + endBlock + ".." + (endBlock + BLOCK) + " does not contain " + expectedEnd);
        }
        Path file = output.resolve(assetKey + ".wav");
        byte[] pcm = device.pcm.toByteArray();
        wav(file, RATE, 2, pcm);
        Map<String, Object> report = map("kind", index == 6 ? "music" : "jingle", "index", index, "group", id,
            "source_sha256", hash(source), "midi_sha256", hash(track.af),
            "published_midi_sha256", hash(expectedMidi),
            "midi_duration_microseconds", sequence.getMicrosecondLength(),
            "frames", frames, "sample_rate", RATE, "channels", 2,
            "engine_end_frame", preview > 0 ? -1 : expectedEnd, "preview", preview > 0,
            "native_eot_observed_block_start", endBlock, "native_eot_observed_block_end", endBlock + BLOCK,
            "original_player_scheduling_used", true,
            "release_tail_frames", preview > 0 ? null : RATE,
            "native_loop_requested", false,
            "native_startup_percussion_channel", 9, "native_startup_percussion_bank", 128,
            "synth_master_volume", 128, "peak_mix_s24", device.peak, "device_clipped_samples", device.clipped,
            "original_device_quantization_verified", true, "pcm_s16le_sha256", hash(pcm),
            "patches", patchNotes, "decoded_samples", decoded.samples,
            "reads", List.of(effects.reads, samples.reads, patches.reads));
        Files.writeString(output.resolve(assetKey + ".json"), JSON.toJson(report) + "\n");
        System.out.println(assetKey + " frames=" + frames + " eot=" + expectedEnd + " peak24=" + device.peak
            + " clipped=" + device.clipped + " samples=" + decoded.samples.size());
    }

    static final class TreeMapSamples
    {
        final Map<String, Integer> references = new java.util.TreeMap<>();
        final Map<String, Object> samples = new java.util.TreeMap<>();
    }

    static void effect(Path root, Path output, int id) throws Exception
    {
        OfflineArchive effects = OfflineArchive.load(root, 4);
        al sound = al.af(effects, id, 0);
        if (sound == null)
        {
            throw new IllegalStateException("Original effect did not load: " + id);
        }
        aj raw = sound.ab();
        ByteBuffer bytes = ByteBuffer.allocate(raw.af.length * 2).order(ByteOrder.LITTLE_ENDIAN);
        for (short value : raw.af)
        {
            bytes.putShort(value);
        }
        wav(output.resolve("sfx-" + id + ".wav"), raw.az, 1, bytes.array());
        var report = rawMetadata(raw);
        report.put("kind", "sfx");
        report.put("index", 4);
        report.put("group", id);
        report.put("reads", effects.reads);
        Files.writeString(output.resolve("sfx-" + id + ".json"), JSON.toJson(report) + "\n");
        System.out.println("sfx-" + id + " frames=" + raw.af.length + " rate=" + raw.az);
    }

    public static void main(String[] args) throws Exception
    {
        if (args.length != 3)
        {
            throw new IllegalArgumentException("SourceAudio verified-input-root jobs.json worktree-output-dir");
        }
        Path root = Path.of(args[0]);
        Path output = Path.of(args[2]);
        Files.createDirectories(output);
        JsonObject request = JSON.fromJson(Files.readString(Path.of(args[1])), JsonObject.class);
        for (var job : request.getAsJsonArray("tracks"))
        {
            music(root, output, job.getAsJsonObject());
        }
        for (var id : request.getAsJsonArray("sound_ids"))
        {
            effect(root, output, id.getAsInt());
        }
    }
}
