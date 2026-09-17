import java.util.Arrays;

/** Read-only transport to original archive accessors, without Archive/network construction. */
final class NativeArchive extends vp
{
    BindingCache cache;
    int index;

    private NativeArchive() { super(null, null, null, 0, false, false, false, false, false); }

    static NativeArchive open(BindingCache cache, int index) throws Exception
    {
        NativeArchive archive = (NativeArchive) NativePolicyProbe.allocate(NativeArchive.class);
        archive.cache = cache;
        archive.index = index;
        var metadata = cache.index(index);
        int[] groups = metadata.getArchives().stream().mapToInt(a -> a.getArchiveId()).toArray();
        int max = Arrays.stream(groups).max().orElse(0);
        va base = archive;
        base.bs = new Object[max + 1][];
        base.bf = new Object[max + 1];
        base.bg = groups;
        base.ba = groups.length * 2034482581;
        base.bt = base.ba;
        archive.ay = index * -50391747;
        for (var group : metadata.getArchives())
        {
            int last = Arrays.stream(group.getFileData()).mapToInt(f -> f.getId()).max().orElse(0);
            base.bs[group.getArchiveId()] = new Object[last + 1];
        }
        return archive;
    }

    byte[] read(int group, int file)
    {
        try
        {
            var value = cache.group(index, group).findFile(file);
            if (value == null) throw new IllegalStateException("Missing native input " + index + "/" + group + "/" + file);
            return value.getContents();
        }
        catch (Exception failure) { throw new IllegalStateException("Read-only native archive failed", failure); }
    }

    @Override public byte[] bd(int group, int file, int guard) { return read(group, file); }
    @Override byte[] bi(int group, int file, int[] keys, int guard)
    {
        if (keys != null) throw new IllegalArgumentException("No encrypted policy inputs are selected.");
        return read(group, file);
    }
    @Override public void ds(int group) { throw new IllegalStateException("Unexpected native archive network request."); }
}
