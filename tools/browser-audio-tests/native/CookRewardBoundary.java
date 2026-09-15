import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/** Original transport boundary only: does not grant XP, complete quests, or choose their selectors. */
public final class CookRewardBoundary
{
    public static void main(String[] args)
    {
        try
        {
            NativePolicyProbe.initialize();
            em.fa((byte) 0);
            cb.hn = SourceAudio.OfflineArchive.load(Path.of(args[0]), 11);
            var cases = new ArrayList<>();
            for (int[] requests : new int[][]{{152,33},{152,34},{34,152},{152,-1}})
            {
                np.ae.clear(); np.ab.clear(); np.ac.clear(); np.aa.clear(); client.ka = false;
                for (int group : requests) NativePolicyProbe.opcode(3202,group,0);
                List<Integer> pending = new ArrayList<>();
                for (Object song : np.ae) pending.add(((nb)song).getArchiveId());
                int expected = requests[1] == -1 ? requests[0] : requests[1];
                NativePolicyProbe.require(pending.equals(List.of(expected)), "Native jingle boundary changed");
                cases.add(SourceAudio.map("submitted",requests,"pending",pending,
                    "level_or_quest_argument_present",false));
            }
            Files.writeString(Path.of(args[1]), SourceAudio.JSON.toJson(SourceAudio.map(
                "schema_version",1,"result","passed","runtime","unmodified injected-client-1.12.38",
                "opcode",3202,"argument_roles",List.of("original_index11_group","unused_auxiliary_integer"),
                "cases",cases,"new_quest_selectors_inferred",false,
                "xp_or_quest_events_generated",false,"external_account",false))+"\n");
        }
        catch(Throwable error)
        {
            error.printStackTrace();
            System.exit(1);
        }
        System.exit(0);
    }
}
