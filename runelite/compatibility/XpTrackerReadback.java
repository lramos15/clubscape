package net.runelite.client.plugins.xptracker;

import net.runelite.api.Skill;

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
}
