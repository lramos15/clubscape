package net.runelite.client.plugins.xptracker;

import net.runelite.api.Skill;
import java.lang.reflect.Field;

/** Read-only assertion plus normal overlay configuration; never writes plugin experience state. */
public final class XpTrackerReadback
{
    public static int gained(XpTrackerPlugin plugin, Skill skill)
    {
        return plugin.getSkillSnapshot(skill).getXpGainedInSession();
    }

    public static void showOverlay(XpTrackerPlugin plugin, Skill skill)
    {
        plugin.addOverlay(skill);
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
            XpState state = (XpState) field.get(plugin);
            return initializationTicksRemaining(plugin) == 0 && state.isInitialized(skill)
                && state.isOverallInitialized() && state.getSkill(skill).getStartXp() == absoluteXp
                && state.getSkill(skill).getCurrentXp() == absoluteXp
                && state.getSkillSnapshot(skill).getXpGainedInSession() == 0;
        }
        catch (ReflectiveOperationException error) { throw new IllegalStateException(error); }
    }
}
