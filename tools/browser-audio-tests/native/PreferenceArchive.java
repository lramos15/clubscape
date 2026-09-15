import java.util.Arrays;

/** Original index/group decoding with only JS5 transport replaced by validated read-only containers. */
final class PreferenceArchive extends vp
{
    final BindingCache source;
    final int index;

    private PreferenceArchive(BindingCache cache, int index) throws Exception
    {
        super(null,null,new vb(),index,false,false,false,false,false);
        source = cache;
        this.index = index;
        source.index(index);
        az(source.container(255,index));
    }

    static PreferenceArchive open(BindingCache cache, int index) throws Exception
    {
        return new PreferenceArchive(cache,index);
    }

    @Override void ab(int group, int guard)
    {
        try
        {
            source.group(index,group);
            byte[] container = source.container(index,group);
            bf[group] = Arrays.copyOf(container,container.length-2);
        }
        catch (Exception failure) { throw new IllegalStateException("Original local group was unavailable",failure); }
    }
}
