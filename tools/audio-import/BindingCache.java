import java.io.IOException;
import java.io.RandomAccessFile;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.Arrays;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.zip.CRC32;
import net.runelite.cache.fs.ArchiveFiles;
import net.runelite.cache.fs.Container;
import net.runelite.cache.fs.Index;
import net.runelite.cache.index.IndexData;

final class BindingCache implements AutoCloseable
{
    final Path root;
    final RandomAccessFile data;
    final Map<Integer, RandomAccessFile> diskIndexes = new LinkedHashMap<>();
    final Map<Integer, Index> indexes = new LinkedHashMap<>();
    final Map<String, Map<String, Object>> groups = new LinkedHashMap<>();

    BindingCache(Path root) throws IOException
    {
        this.root = root;
        // RuneLite's DiskStorage opens files read/write; this reader never opens a write handle.
        data = new RandomAccessFile(root.resolve("main_file_cache.dat2").toFile(), "r");
    }

    static String hash(byte[] bytes) throws Exception
    {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(bytes));
    }

    static Map<String, Object> map(Object... pairs)
    {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < pairs.length; i += 2) result.put((String) pairs[i], pairs[i + 1]);
        return result;
    }

    static int medium(RandomAccessFile file) throws IOException
    {
        return file.readUnsignedByte() << 16 | file.readUnsignedByte() << 8 | file.readUnsignedByte();
    }

    byte[] container(int index, int group) throws IOException
    {
        if (index < 0 || index > 255 || group < 0) throw new IOException("Invalid source group identity");
        RandomAccessFile table = diskIndexes.get(index);
        if (table == null)
        {
            table = new RandomAccessFile(root.resolve("main_file_cache.idx" + index).toFile(), "r");
            diskIndexes.put(index, table);
        }
        if (group * 6L + 6 > table.length()) throw new IOException("Missing source group " + index + "/" + group);
        table.seek(group * 6L);
        int size = medium(table), sector = medium(table);
        if (size <= 0 || size > 128 * 1024 * 1024 || sector <= 0) throw new IOException("Invalid source index entry");
        byte[] result = new byte[size];
        int offset = 0, part = 0;
        while (offset < size)
        {
            int header = group > 65535 ? 10 : 8;
            int count = Math.min(520 - header, size - offset);
            if (sector <= 0 || sector * 520L + header + count > data.length())
                throw new IOException("Invalid source sector " + index + "/" + group + "/" + part);
            data.seek(sector * 520L);
            int actualGroup = group > 65535 ? data.readInt() : data.readUnsignedShort();
            int actualPart = data.readUnsignedShort(), next = medium(data), actualIndex = data.readUnsignedByte();
            if (actualGroup != group || actualPart != part || actualIndex != index)
                throw new IOException("Source sector identity mismatch");
            data.readFully(result, offset, count);
            offset += count;
            sector = next;
            part++;
        }
        if (sector != 0) throw new IOException("Unexpected continuation after complete source group");
        return result;
    }

    Index index(int id) throws Exception
    {
        if (indexes.containsKey(id)) return indexes.get(id);
        byte[] bytes = container(255, id);
        Container unpacked = Container.decompress(bytes, null);
        IndexData metadata = new IndexData();
        metadata.load(unpacked.data);
        Index index = new Index(id);
        index.setProtocol(metadata.getProtocol());
        index.setRevision(metadata.getRevision());
        index.setNamed(metadata.isNamed());
        index.setSized(metadata.isSized());
        index.setCrc(unpacked.crc);
        index.setCompression(unpacked.compression);
        for (var entry : metadata.getArchives())
        {
            var archive = index.addArchive(entry.getId());
            archive.setNameHash(entry.getNameHash());
            archive.setCrc(entry.getCrc());
            archive.setRevision(entry.getRevision());
            archive.setCompressedSize(entry.getCompressedSize());
            archive.setDecompressedSize(entry.getDecompressedSize());
            archive.setFileData(entry.getFiles());
        }
        indexes.put(id, index);
        groups.put("255/" + id, map("index", 255, "group", id, "container_sha256", hash(bytes),
            "container_size_bytes", bytes.length, "revision", metadata.getRevision(),
            "groups", metadata.getArchives().length, "named", metadata.isNamed()));
        return index;
    }

    ArchiveFiles group(int indexId, int groupId) throws Exception
    {
        var archive = index(indexId).getArchive(groupId);
        if (archive == null) throw new IOException("Missing original group " + indexId + "/" + groupId);
        byte[] bytes = container(indexId, groupId);
        if (bytes.length < 7) throw new IOException("Truncated source container");
        int length = ByteBuffer.wrap(bytes, 1, 4).getInt();
        long end = 5L + length + (bytes[0] == 0 ? 0 : 4);
        if (length < 0 || end + 2 != bytes.length
            || (ByteBuffer.wrap(bytes, bytes.length - 2, 2).getShort() & 65535) != (archive.getRevision() & 65535))
            throw new IOException("Source container size/revision mismatch");
        CRC32 crc = new CRC32();
        crc.update(bytes, 0, (int) end);
        if (crc.getValue() != Integer.toUnsignedLong(archive.getCrc())) throw new IOException("Source CRC mismatch");
        ArchiveFiles files = archive.getFiles(Arrays.copyOf(bytes, (int) end));
        groups.put(indexId + "/" + groupId, map("index", indexId, "group", groupId,
            "revision", archive.getRevision(), "crc32", crc.getValue(), "name_hash", archive.getNameHash(),
            "container_sha256", hash(bytes), "container_size_bytes", bytes.length,
            "file_ids", files.getFiles().stream().map(file -> file.getFileId()).toList()));
        return files;
    }

    @Override public void close() throws IOException
    {
        try
        {
            for (var file : diskIndexes.values()) file.close();
        }
        finally
        {
            data.close();
        }
    }
}
