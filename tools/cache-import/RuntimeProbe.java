import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.net.URL;
import java.nio.charset.Charset;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.LinkedHashMap;
import java.util.Map;
import net.runelite.api.Client;
import net.runelite.api.ClientConfiguration;
import net.runelite.api.hooks.Callbacks;

/** Runs configuration initialization only; never accepts terms, logs in, or renders a scene. */
public final class RuntimeProbe
{
    public static void main(String[] args) throws Exception
    {
        if (args.length != 1)
        {
            throw new IllegalArgumentException("Usage: RuntimeProbe jav_config.ws");
        }
        Map<String, String> parameters = new LinkedHashMap<>();
        String codebase = null;
        for (String line : Files.readAllLines(Path.of(args[0]), Charset.forName("windows-1252")))
        {
            if (line.startsWith("param="))
            {
                String[] entry = line.substring(6).split("=", 2);
                parameters.put(entry[0], entry[1]);
            }
            else if (line.startsWith("codebase="))
            {
                codebase = line.substring(9);
            }
        }
        URL base = new URL(codebase);
        Class<?> implementation = Class.forName("client");
        Client client = (Client) implementation.getDeclaredConstructor().newInstance();
        client.setConfiguration(new ClientConfiguration()
        {
            public URL getCodeBase() { return base; }
            public String getParameter(String name) { return parameters.get(name); }
            public void onError(String code) { System.err.println("SOURCE_CLIENT_ERROR=" + code); }
        });
        Callbacks callbacks = (Callbacks) Proxy.newProxyInstance(
            Callbacks.class.getClassLoader(), new Class<?>[]{Callbacks.class},
            (proxy, method, values) ->
            {
                Class<?> type = method.getReturnType();
                if (type == boolean.class) return false;
                if (type == int.class) return 0;
                if (type == long.class) return 0L;
                return null;
            });
        for (var field : implementation.getDeclaredFields())
        {
            if (field.getType() == Callbacks.class)
            {
                field.setAccessible(true);
                field.set(client, callbacks);
            }
        }
        int before = client.getRevision();
        String initError = null;
        try
        {
            implementation.getMethod("init").invoke(client);
        }
        catch (InvocationTargetException ex)
        {
            initError = ex.getCause().getClass().getName() + ": " + ex.getCause().getMessage();
        }
        int after = client.getRevision();
        System.out.println("BUILD_ID=" + client.getBuildID());
        System.out.println("OFFICIAL_CONFIG_REVISION=" + parameters.get("25"));
        System.out.println("REVISION_BEFORE_INIT=" + before);
        System.out.println("REVISION_AFTER_INIT=" + after);
        System.out.println("INIT_EXCEPTION=" + initError);
        System.out.println("USER_HOME=" + System.getProperty("user.home"));
        System.out.println("CAPTURE_OR_GAMEPLAY_VERIFIED=false");
        System.out.flush();
        System.exit(initError == null && after == Integer.parseInt(parameters.get("25")) ? 0 : 1);
    }
}
