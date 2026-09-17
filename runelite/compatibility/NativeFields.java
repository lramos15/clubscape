import java.lang.reflect.Field;
import java.math.BigInteger;

/** Exact injected1.12.38 bindings; taken from the verified original-renderer capture path. */
final class NativeFields
{
    static Field field(Class<?> owner, String name, Class<?> type) throws Exception
    {
        for (Field field : owner.getDeclaredFields())
        {
            if (field.getName().equals(name) && field.getType() == type)
            {
                field.setAccessible(true);
                return field;
            }
        }
        throw new IllegalStateException("Pinned native member missing: " + owner.getName() + "." + name);
    }

    static void put(Object target, Class<?> owner, String name, Class<?> type, Object value) throws Exception
    {
        field(owner, name, type).set(target, value);
    }

    static void logicalInt(Object target, Class<?> owner, String name, int multiplier, int value) throws Exception
    {
        int inverse = BigInteger.valueOf(Integer.toUnsignedLong(multiplier))
            .modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
        field(owner, name, int.class).setInt(target, value * inverse);
    }

    static void logicalLong(Object target, Class<?> owner, String name, long multiplier, long value) throws Exception
    {
        long inverse = new BigInteger(Long.toUnsignedString(multiplier))
            .modInverse(BigInteger.ONE.shiftLeft(64)).longValue();
        field(owner, name, long.class).setLong(target, value * inverse);
    }

    static int[] path(Object actor, String name) throws Exception
    {
        for (var method : dh.class.getDeclaredMethods())
        {
            if (method.getName().equals(name) && method.getReturnType() == int[].class
                && method.getParameterCount() == 0)
            {
                method.setAccessible(true);
                return (int[]) method.invoke(actor);
            }
        }
        throw new IllegalStateException("Pinned native actor path accessor missing: " + name);
    }
}
