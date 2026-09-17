import net.runelite.api.Skill;
import net.runelite.client.plugins.xptracker.XpTrackerPlugin;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.awt.Component;
import java.awt.Container;
import java.util.ArrayList;
import java.util.List;
import javax.swing.JLabel;

/** Read-only assertion plus normal overlay configuration; never writes plugin experience state. */
public final class XpTrackerReadback
{
    private static Object invoke(Object receiver, String name, Class<?>[] parameters, Object... arguments)
    {
        try
        {
            Method method = receiver.getClass().getDeclaredMethod(name, parameters);
            method.setAccessible(true);
            return method.invoke(receiver, arguments);
        }
        catch (ReflectiveOperationException error) { throw new IllegalStateException(error); }
    }

    public static int gained(XpTrackerPlugin plugin, Skill skill)
    {
        Object snapshot = invoke(plugin, "getSkillSnapshot", new Class<?>[]{Skill.class}, skill);
        return ((Number) invoke(snapshot, "getXpGainedInSession", new Class<?>[0])).intValue();
    }

    public static void showOverlay(XpTrackerPlugin plugin, Skill skill)
    {
        invoke(plugin, "addOverlay", new Class<?>[]{Skill.class}, skill);
    }

    public static int initializationTicksRemaining(XpTrackerPlugin plugin)
    {
        try
        {
            Field field = XpTrackerPlugin.class.getDeclaredField("initializeTracker");
            field.setAccessible(true);
            return field.getInt(plugin);
        }
        catch (ReflectiveOperationException error) { throw new IllegalStateException(error); }
    }

    public static boolean baselineMatches(XpTrackerPlugin plugin, Skill skill, int absoluteXp)
    {
        try
        {
            Field field = XpTrackerPlugin.class.getDeclaredField("xpState");
            field.setAccessible(true);
            Object state = field.get(plugin);
            if (state == null || initializationTicksRemaining(plugin) != 0
                || !(boolean) invoke(state, "isInitialized", new Class<?>[]{Skill.class}, skill)
                || !(boolean) invoke(state, "isOverallInitialized", new Class<?>[0]))
                return false;
            Object skillState = invoke(state, "getSkill", new Class<?>[]{Skill.class}, skill);
            return ((Number) invoke(skillState, "getStartXp", new Class<?>[0])).longValue() == absoluteXp
                && ((Number) invoke(skillState, "getCurrentXp", new Class<?>[0])).longValue() == absoluteXp
                && gained(plugin, skill) == 0;
        }
        catch (ReflectiveOperationException error) { throw new IllegalStateException(error); }
    }

    public static List<String> displayedLabels(XpTrackerPlugin plugin)
    {
        try
        {
            Field field = XpTrackerPlugin.class.getDeclaredField("xpPanel");
            field.setAccessible(true);
            List<String> result = new ArrayList<>();
            labels((Component) field.get(plugin), result);
            return result;
        }
        catch (ReflectiveOperationException error) { throw new IllegalStateException(error); }
    }

    private static void labels(Component component, List<String> result)
    {
        if (component instanceof JLabel)
        {
            String text = ((JLabel) component).getText();
            if (text != null && !text.isBlank()) result.add(text.replaceAll("<[^>]+>", ""));
        }
        if (component instanceof Container)
            for (Component child : ((Container) component).getComponents()) labels(child, result);
    }
}
