import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;

/**
 * Little-endian chunked container: magic "CSRC", u32 version, then repeated
 * [4 ASCII tag bytes][u32 payload length][payload]. Readers ignore unknown tags.
 */
final class ChunkWriter
{
    static final int VERSION = 1;
    private final ByteArrayOutputStream bytes = new ByteArrayOutputStream();

    ChunkWriter()
    {
        bytes.writeBytes("CSRC".getBytes(StandardCharsets.US_ASCII));
        bytes.writeBytes(le(4).putInt(VERSION).array());
    }

    private static ByteBuffer le(int size)
    {
        return ByteBuffer.allocate(size).order(ByteOrder.LITTLE_ENDIAN);
    }

    private void chunk(String tag, byte[] payload)
    {
        if (tag.length() != 4) throw new IllegalArgumentException("tag " + tag);
        bytes.writeBytes(tag.getBytes(StandardCharsets.US_ASCII));
        bytes.writeBytes(le(4).putInt(payload.length).array());
        bytes.writeBytes(payload);
    }

    ChunkWriter ints(String tag, int... values)
    {
        ByteBuffer buffer = le(values.length * 4);
        for (int value : values) buffer.putInt(value);
        chunk(tag, buffer.array());
        return this;
    }

    ChunkWriter longs(String tag, long... values)
    {
        ByteBuffer buffer = le(values.length * 8);
        for (long value : values) buffer.putLong(value);
        chunk(tag, buffer.array());
        return this;
    }

    ChunkWriter floats(String tag, float[] values, int count)
    {
        ByteBuffer buffer = le(count * 4);
        for (int i = 0; i < count; i++) buffer.putFloat(values[i]);
        chunk(tag, buffer.array());
        return this;
    }

    ChunkWriter ints(String tag, int[] values, int count)
    {
        ByteBuffer buffer = le(count * 4);
        for (int i = 0; i < count; i++) buffer.putInt(values[i]);
        chunk(tag, buffer.array());
        return this;
    }

    ChunkWriter shorts(String tag, short[] values, int count)
    {
        ByteBuffer buffer = le(count * 2);
        for (int i = 0; i < count; i++) buffer.putShort(values[i]);
        chunk(tag, buffer.array());
        return this;
    }

    ChunkWriter bytes(String tag, byte[] values, int count)
    {
        chunk(tag, java.util.Arrays.copyOf(values, count));
        return this;
    }

    /** Jagged int arrays: u32 outer count, then for each: u32 length + ints. */
    ChunkWriter jagged(String tag, int[][] values)
    {
        int size = 4;
        for (int[] row : values) size += 4 + (row == null ? 0 : row.length) * 4;
        ByteBuffer buffer = le(size);
        buffer.putInt(values.length);
        for (int[] row : values)
        {
            if (row == null) { buffer.putInt(0); continue; }
            buffer.putInt(row.length);
            for (int value : row) buffer.putInt(value);
        }
        chunk(tag, buffer.array());
        return this;
    }

    ChunkWriter text(String tag, String value)
    {
        chunk(tag, value.getBytes(StandardCharsets.UTF_8));
        return this;
    }

    byte[] toBytes()
    {
        return bytes.toByteArray();
    }

    /** Serialized chunks with the named tags removed (header kept); used for shape hashing. */
    byte[] toBytesExcept(String... tags)
    {
        java.util.Set<String> skip = java.util.Set.of(tags);
        byte[] data = toBytes();
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        out.write(data, 0, 8);
        int offset = 8;
        while (offset + 8 <= data.length)
        {
            String tag = new String(data, offset, 4, StandardCharsets.US_ASCII);
            int length = ByteBuffer.wrap(data, offset + 4, 4).order(ByteOrder.LITTLE_ENDIAN).getInt();
            if (!skip.contains(tag)) out.write(data, offset, 8 + length);
            offset += 8 + length;
        }
        return out.toByteArray();
    }

    /** Payload of the first chunk with `tag` in a serialized chunk file, or null. */
    static byte[] chunkOf(byte[] data, String tag)
    {
        int offset = 8;
        while (offset + 8 <= data.length)
        {
            String current = new String(data, offset, 4, StandardCharsets.US_ASCII);
            int length = ByteBuffer.wrap(data, offset + 4, 4).order(ByteOrder.LITTLE_ENDIAN).getInt();
            if (current.equals(tag)) return java.util.Arrays.copyOfRange(data, offset + 8, offset + 8 + length);
            offset += 8 + length;
        }
        return null;
    }

    /** Appends the named chunks of another writer verbatim. */
    ChunkWriter copyChunks(ChunkWriter source, String... tags)
    {
        java.util.Set<String> wanted = java.util.Set.of(tags);
        byte[] data = source.toBytes();
        int offset = 8;
        while (offset + 8 <= data.length)
        {
            String tag = new String(data, offset, 4, StandardCharsets.US_ASCII);
            int length = ByteBuffer.wrap(data, offset + 4, 4).order(ByteOrder.LITTLE_ENDIAN).getInt();
            if (wanted.contains(tag)) bytes.write(data, offset, 8 + length);
            offset += 8 + length;
        }
        return this;
    }

    String write(Path file) throws Exception
    {
        Files.createDirectories(file.getParent());
        byte[] data = toBytes();
        Files.write(file, data);
        return java.util.HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
    }

    static String sha256(byte[] data) throws Exception
    {
        return java.util.HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
    }

    static String sha256(Path file) throws IOException
    {
        try
        {
            return sha256(Files.readAllBytes(file));
        }
        catch (Exception error)
        {
            throw new IOException(error);
        }
    }
}
