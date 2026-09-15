import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;

/** Original document widgets/models with explicit comparison text, never client account policy. */
public final class UiDocumentCapture
{
    static void run(OriginalCapture capture, HudCapture hud) throws Exception
    {
        var records = new ArrayList<Object>();
        UiAssetExport.sprite(capture, 2385);
        UiAssetExport.sprite(capture, 2389);
        UiModeCapture.tab(hud, 3);
        var book = capture.game.openInterface(161 << 16 | 16, 392, 0);
        UiModeCapture.load(hud, 392);
        capture.game.getWidget(392, 11).setHidden(true);
        capture.game.getWidget(392, 27).setHidden(true);
        capture.game.getWidget(392, 43).setHidden(false);
        capture.game.getWidget(392, 59).setHidden(false);
        for (int page = 0; page < 2; page++)
        {
            String name = page == 0 ? "ui4-book-first" : "ui4-book-last";
            String title = "Source-only book title";
            var lines = new ArrayList<String>();
            capture.game.getWidget(392, 6).setText(title);
            for (int line = 0; line < 30; line++)
            {
                String text = "Source-only line " + (page * 30 + line + 1);
                lines.add(text);
                capture.game.getWidget(392, line < 15 ? 44 + line : 60 + line - 15).setText(text);
            }
            capture.game.getWidget(392, 9).setText(Integer.toString(page * 2 + 1));
            capture.game.getWidget(392, 10).setText(Integer.toString(page * 2 + 2));
            capture.game.getWidget(392, 75).setHidden(page == 0);
            capture.game.getWidget(392, 77).setHidden(page == 1);
            capture.game.runScript(907, 161 << 16, 1130);
            UiModeCapture.frame(hud, name, 392);
            records.add(OriginalCapture.map("case", name, "group", 392, "title", title, "page", page,
                "lines", lines, "previous", page != 0, "next", page != 1,
                "scope", "Explicit original facing-page text/visibility inputs, not production book content or account policy."));
        }
        capture.game.closeInterface(book, true);
        var map = capture.game.openInterface(161 << 16 | 16, 615, 0);
        UiModeCapture.load(hud, 615);
        capture.game.runScript(907, 161 << 16, 1130);
        var point = capture.game.getLocalPlayer().getWorldLocation();
        var tile = OriginalCapture.map("x", point.getX(), "y", point.getY(), "plane", point.getPlane());
        var marker = capture.game.getWidget(615, 0).getChild(0);
        if (marker == null) throw new IllegalStateException("Original newcomer-map player marker was not created.");
        System.out.println("SOURCE_MAP_PLAYER " + point + " base=" + capture.game.getBaseX() + "," + capture.game.getBaseY()
            + " markerHidden=" + marker.isHidden() + " model=" + marker.getModelId());
        UiModeCapture.frame(hud, "ui4-map-tutors-hidden", 615);
        records.add(OriginalCapture.map("case", "ui4-map-tutors-hidden", "group", 615, "tutors", false,
            "tile", tile, "markerHidden", marker.isHidden()));
        UiModeCapture.op(capture, capture.game.getWidget(615, 18), 1);
        capture.game.runScript(907, 161 << 16, 1130);
        UiModeCapture.frame(hud, "ui4-map-tutors-shown", 615);
        records.add(OriginalCapture.map("case", "ui4-map-tutors-shown", "group", 615, "tutors", true,
            "tile", tile, "markerHidden", marker.isHidden(),
            "operation", List.of(615, 18, 1), "sourceScripts", List.of(2039, 2040, 2041, 2042, 2043)));
        int oldX = marker.getOriginalX(), oldY = marker.getOriginalY();
        boolean hidden = marker.isSelfHidden();
        marker.setOriginalX(240).setOriginalY(160).setHidden(false);
        marker.revalidate();
        UiAssetExport.staticModels(capture, hud);
        Files.writeString(capture.output.resolve("document-marker.json"), OriginalCapture.JSON.toJson(
            OriginalCapture.map("widget", UiAssetExport.widget(marker), "animation", marker.getAnimationId(),
                "scope", "Original model-only glyph at an explicit interior position; not a source visibility or animation-sequence claim.")));
        marker.setOriginalX(oldX).setOriginalY(oldY).setHidden(hidden);
        marker.revalidate();
        capture.game.closeInterface(map, true);
        Files.writeString(capture.output.resolve("document-inputs.json"), OriginalCapture.JSON.toJson(records));
    }
}
