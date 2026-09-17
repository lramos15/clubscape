import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

/** Original source audio preference widgets, with explicit test-only percentage inputs. */
public final class UiAudioCapture
{
    static void run(OriginalCapture capture, HudCapture hud) throws Exception
    {
        int[] varps = {3796, 168, 169, 872, 5588};
        int[] before = new int[varps.length];
        for (int index = 0; index < varps.length; index++) before[index] = capture.game.getVarps()[varps[index]];
        int page = capture.game.getVarbitValue(9683);
        List<Object> records = new ArrayList<>();
        String[] names = {"default", "master-0", "master-50", "music-0", "music-50", "effects-0", "area-0", "mixed", "channel-boundaries"};
        int[][] values = {{100,100,100,100}, {0,100,100,100}, {50,100,100,100}, {100,0,100,100},
            {100,50,100,100}, {100,100,0,100}, {100,100,100,0}, {37,21,66,83}, {100,1,99,25}};
        for (int index = 0; index < values.length; index++)
        {
            for (int channel = 0; channel < 4; channel++) capture.game.getVarps()[varps[channel]] = values[index][channel];
            capture.game.getVarps()[5588] = 0;
            capture.game.setVarbit(9683, 1);
            UiModeCapture.load(hud, 116);
            UiModeCapture.tab(hud, 11);
            capture.game.runScript(907, 161 << 16, 1130);
            String name = "native-audio-" + names[index];
            UiModeCapture.frame(hud, name, 116);
            records.add(OriginalCapture.map("case", name, "percentages", values[index],
                "varps", varps, "sourceAreaOverride", 0, "sourcePageVarbit9683", 1,
                "scope", "Explicit original UI preference fields; not a scene-varp bridge, production balance or audible playback proof."));
        }
        for (int index = 0; index < varps.length; index++) capture.game.getVarps()[varps[index]] = before[index];
        capture.game.setVarbit(9683, page);
        Files.writeString(capture.output.resolve("audio-ui-inputs.json"), OriginalCapture.JSON.toJson(records));
    }
}
