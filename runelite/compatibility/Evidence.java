import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import java.io.BufferedWriter;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.Map;

final class Evidence implements AutoCloseable
{
    static final Gson JSON = new GsonBuilder().disableHtmlEscaping().create();
    private final BufferedWriter output;

    Evidence(Path path) throws IOException
    {
        output = Files.newBufferedWriter(path);
    }

    static Map<String, Object> fields(Object... values)
    {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < values.length; i += 2) result.put((String) values[i], values[i + 1]);
        return result;
    }

    synchronized void record(String kind, Object... values)
    {
        Map<String, Object> event = fields("at", Instant.now().toString(), "kind", kind);
        event.putAll(fields(values));
        try
        {
            output.write(JSON.toJson(event));
            output.newLine();
            output.flush();
        }
        catch (IOException error)
        {
            throw new IllegalStateException("Cannot preserve integration evidence", error);
        }
    }

    public void close() throws IOException { output.close(); }
}
