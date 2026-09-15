import net.runelite.api.Skill;
import net.runelite.client.plugins.xptracker.XpTrackerPlugin;

/** Class-loading/read-only boundary check; no synthetic XP, callbacks, native client, or server. */
public final class PluginBoundaryChecks
{
    public static void main(String[] args)
    {
        if (XpTrackerPlugin.class.getPackageName().equals(XpTrackerReadback.class.getPackageName()))
            throw new AssertionError("Adapter class must not share the signed upstream package");
        Object[] signers = XpTrackerPlugin.class.getSigners();
        if (signers == null || signers.length == 0)
            throw new AssertionError("Pinned upstream plugin signature was unexpectedly removed");
        XpTrackerPlugin plugin = new XpTrackerPlugin();
        if (XpTrackerReadback.initializationTicksRemaining(plugin) != 0
            || XpTrackerReadback.baselineMatches(plugin, Skill.FISHING, 0))
            throw new AssertionError("Uninitialized plugin cannot be credited as a valid live baseline");
        System.out.println("{\"signed_plugin_boundary_checks\":3,\"xp_callbacks\":0,\"live_compatibility_proof\":false}");
    }
}
