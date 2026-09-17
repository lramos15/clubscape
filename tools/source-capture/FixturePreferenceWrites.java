import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ScheduledThreadPoolExecutor;

/** Records the identified native UI-preference save operation without touching persisted user settings. */
public final class FixturePreferenceWrites extends ScheduledThreadPoolExecutor
{
    final List<String> requests = new ArrayList<>();

    public FixturePreferenceWrites() { super(0); }

    @Override
    public void execute(Runnable command)
    {
        String source = command.getClass().getName();
        if (!source.startsWith("mw$$Lambda$"))
        {
            throw new IllegalStateException("Unexpected source async operation in HUD fixture: " + source);
        }
        requests.add("mw.af -> serialized native ClientPreferences save");
        System.out.println("NATIVE_PREFERENCE_SAVE_RECORDED count=" + requests.size() + " persistence deliberately disabled");
    }
}
