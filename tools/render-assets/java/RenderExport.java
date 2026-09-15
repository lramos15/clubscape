import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import net.runelite.api.Model;
import net.runelite.api.ModelData;
import net.runelite.api.Perspective;

/**
 * Exports neutral geometry/material buffers from the unmodified pinned original runtime
 * for the ClubScape renderer. Nothing here renders; the original data structures are read
 * after the original code has built them (lighting, recolors, texture decoding, scenes).
 */
public final class RenderExport
{
    static final double BRIGHTNESS = 0.8;
    static final int TEXTURE_SIZE = 128;
    final OriginalCapture runtime;
    final OriginalCache cache;
    final Path output;
    final Map<String, Object> manifest = new LinkedHashMap<>();
    final Map<String, Object> files = new LinkedHashMap<>();

    public static void main(String[] args)
    {
        try
        {
            java.lang.reflect.Field randomField = Class.forName("java.lang.Math$RandomNumberGeneratorHolder").getDeclaredField("randomNumberGenerator");
            randomField.setAccessible(true);
            ((java.util.Random) randomField.get(null)).setSeed(0L);
            try (OriginalCache cache = new OriginalCache(Path.of(args[0])))
            {
                Path output = Path.of(args[1]);
                String profile = args.length > 2 ? args[2] : "all";
                RenderExport export = new RenderExport(cache, output);
                export.run(profile, args);
            }
            System.exit(0);
        }
        catch (Throwable error)
        {
            error.printStackTrace();
            System.exit(1);
        }
    }

    RenderExport(OriginalCache cache, Path output) throws Exception
    {
        this.cache = cache;
        this.output = output;
        Files.createDirectories(output);
        // The capture bootstrap binds the original archives, revision, texture provider
        // (brightness 0.8, 128px textures), palette brightness and fonts.
        runtime = new OriginalCapture(cache, output.resolve(".capture-scratch"));
        manifest.put("schema_version", 1);
        manifest.put("kind", "clubscape_render_assets");
        manifest.put("source_runtime", "injected-client-1.12.38");
        manifest.put("source_revision", runtime.game.getRevision());
        manifest.put("source_cache_id", 2695);
        manifest.put("brightness", BRIGHTNESS);
        manifest.put("texture_size", TEXTURE_SIZE);
        manifest.put("units_per_tile", 128);
        manifest.put("scene_angle_units_per_turn", 16384);
        manifest.put("model_angle_units_per_turn", 2048);
        manifest.put("classification", "Neutral buffers read from original runtime structures; not a candidate render and not a source reference image.");
    }

    @SuppressWarnings("unchecked")
    void loadExistingManifest() throws Exception
    {
        Path existing = output.resolve("manifest.json");
        if (!Files.exists(existing)) return;
        Map<String, Object> previous = OriginalCapture.JSON.fromJson(Files.readString(existing), Map.class);
        for (Map.Entry<String, Object> entry : previous.entrySet())
        {
            if (entry.getKey().equals("files"))
            {
                for (Map.Entry<String, Object> file : ((Map<String, Object>) entry.getValue()).entrySet())
                {
                    if (Files.exists(output.resolve(file.getKey()))) files.put(file.getKey(), file.getValue());
                }
            }
            else if (!manifest.containsKey(entry.getKey())) manifest.put(entry.getKey(), entry.getValue());
        }
    }

    void run(String profile, String[] args) throws Exception
    {
        loadExistingManifest();
        boolean all = profile.equals("all");
        if (all || profile.equals("tables")) exportTables();
        if (all || profile.equals("palette")) exportPalette();
        if (all || profile.equals("textures")) exportTextures();
        if (all || profile.equals("models")) exportFixtureModels();
        if (all || profile.equals("npcs")) exportNpcs();
        if (all || profile.equals("scenes")) new SceneExport(this).run();
        manifest.put("files", files);
        Files.writeString(output.resolve("manifest.json"), OriginalCapture.JSON.toJson(manifest) + "\n");
        System.out.println("RENDER_EXPORT_OK files=" + files.size());
    }

    void record(String name, String sha256, long size, Object detail)
    {
        files.put(name, OriginalCapture.map("sha256", sha256, "size_bytes", size, "detail", detail));
    }

    void exportTables() throws Exception
    {
        ChunkWriter writer = new ChunkWriter();
        writer.ints("SIN2", Perspective.SINE, 2048).ints("COS2", Perspective.COSINE, 2048);
        writer.floats("SNF2", Perspective.SINEF, 2048).floats("CSF2", Perspective.COSINEF, 2048);
        writer.ints("SN14", Perspective.SINE14, 16384).ints("CS14", Perspective.COSINE14, 16384);
        writer.floats("SF14", Perspective.SINEF14, 16384).floats("CF14", Perspective.COSINEF14, 16384);
        writer.ints("RCP1", fh.ag, fh.ag.length);
        if (fh.as != Perspective.SINE || fh.ax != Perspective.COSINE || fx.am != fh.as || gb.as != Perspective.SINE14)
        {
            throw new IllegalStateException("Original trig tables are not the RuneLite Perspective tables");
        }
        Path file = output.resolve("tables.bin");
        String sha = writer.write(file);
        record("tables.bin", sha, Files.size(file), OriginalCapture.map("source", "net.runelite.api.Perspective tables bound into fh/gb/fx",
            "unit_2048", 0.0030679615, "unit_16384", 3.834951969714103E-4));
        System.out.println("TABLES sha=" + sha);
    }

    void exportPalette() throws Exception
    {
        int[] palette = fh.ae;
        if (palette.length != 65536) throw new IllegalStateException("Palette size");
        if (fh.as() != BRIGHTNESS) throw new IllegalStateException("Palette brightness " + fh.as());
        int nonzero = 0;
        for (int value : palette) if (value != 0) nonzero++;
        if (nonzero < 60000) throw new IllegalStateException("Palette not initialized");
        ChunkWriter writer = new ChunkWriter();
        writer.ints("BRGT", Float.floatToIntBits((float) BRIGHTNESS)).ints("PLTE", palette, palette.length);
        Path file = output.resolve("palette.bin");
        String sha = writer.write(file);
        record("palette.bin", sha, Files.size(file), OriginalCapture.map("brightness", BRIGHTNESS, "entries", 65536,
            "source", "fh.ae after original fh.ae(0.8) HSL palette build"));
        System.out.println("PALETTE sha=" + sha);
    }

    void exportTextures() throws Exception
    {
        ec provider = (ec) fh.ao.ai;
        if (provider == null) throw new IllegalStateException("Texture provider not bound");
        List<Object> textures = new ArrayList<>();
        for (int id = 0; id < provider.az.length; id++)
        {
            fu texture = provider.az[id];
            if (texture == null) continue;
            int[] pixels = provider.load(id);
            if (pixels == null) throw new IllegalStateException("Texture " + id + " did not load");
            if (pixels.length != TEXTURE_SIZE * TEXTURE_SIZE) throw new IllegalStateException("Texture size " + id + " " + pixels.length);
            ChunkWriter writer = new ChunkWriter();
            writer.ints("TXHD", id, TEXTURE_SIZE, texture.ax, texture.ac ? 1 : 0, texture.aa, texture.ao, texture.as);
            writer.ints("TXPX", pixels, pixels.length);
            Path file = output.resolve("textures/" + id + ".bin");
            String sha = writer.write(file);
            record("textures/" + id + ".bin", sha, Files.size(file), OriginalCapture.map("texture_id", id, "average_rgb", texture.ax,
                "transparent", texture.ac, "animation_direction", texture.aa, "animation_speed", texture.ao, "sprite_group", texture.as));
            textures.add(id);
        }
        if (textures.size() < 30) throw new IllegalStateException("Too few textures " + textures.size());
        manifest.put("textures", textures);
        System.out.println("TEXTURES count=" + textures.size());
    }

    static int decodedEd(fx model)
    {
        return model.ed * -1256242689;
    }

    /** Serializes a lit original Model exactly as the original draw path consumes it. */
    static ChunkWriter model(fx model)
    {
        if (model.by <= 0 || model.bd <= 0) throw new IllegalStateException("Empty native model");
        fx.et(model); // cylinder bounds (cf=1) as used by the scene draw path
        int stateAfterCylinder = model.cf;
        int cg1 = model.cg, ch1 = model.ch, cn1 = model.cn, ed1 = decodedEd(model), cz1 = model.cz;
        ChunkWriter writer = new ChunkWriter();
        writer.ints("MDHD", model.by, model.bd, model.cv, model.cm, model.cd, model.cc ? 1 : 0, model.ce == null ? 0 : model.ce.as, stateAfterCylinder, model.dc);
        writer.ints("BNDC", cg1, ch1, cn1, ed1, cz1);
        writer.floats("VRTX", model.wh, model.by).floats("VRTY", model.pa, model.by).floats("VRTZ", model.mn, model.by);
        writer.ints("FIDA", model.bl, model.bd).ints("FIDB", model.bv, model.bd).ints("FIDC", model.bh, model.bd);
        writer.ints("FCLA", model.bz, model.bd).ints("FCLB", model.cr, model.bd).ints("FCLC", model.cu, model.bd);
        if (model.cq != null) writer.shorts("FTEX", model.cq, model.bd);
        if (model.cp != null) writer.bytes("FTXC", model.cp, model.bd);
        if (model.cs != null && model.cv > 0)
        {
            writer.ints("TXPI", model.cs, model.cv).ints("TXMI", model.cy, model.cv).ints("TXNI", model.co, model.cv);
        }
        if (model.cb != null) writer.bytes("FPRI", model.cb, model.bd);
        if (model.ct != null) writer.bytes("FALP", model.ct, model.bd);
        if (model.cl != null) writer.bytes("FBIA", model.cl, model.bd);
        if (model.cj != null) writer.jagged("VGRP", model.cj);
        if (model.ck != null) writer.jagged("FGRP", model.ck);
        if (model.cw != null) writer.jagged("VGR2", model.cw);
        if (model.ca != null) writer.jagged("FGR2", model.ca);
        return writer;
    }

    String writeModel(String name, fx model, Object detail) throws Exception
    {
        Path file = output.resolve("models/" + name + ".bin");
        String sha = model(model).write(file);
        record("models/" + name + ".bin", sha, Files.size(file), OriginalCapture.map("vertices", model.by, "faces", model.bd,
            "textured_faces", model.cq == null ? 0 : countTextured(model), "detail", detail));
        return sha;
    }

    static int countTextured(fx model)
    {
        int count = 0;
        for (int i = 0; i < model.bd; i++) if (model.cq[i] != -1) count++;
        return count;
    }

    void exportFixtureModels() throws Exception
    {
        ModelData tree = runtime.game.loadModelData(1570);
        if (tree.getVerticesCount() != 90 || tree.getFaceCount() != 110) throw new IllegalStateException("Original tree topology mismatch");
        tree.recolor((short) 3470, (short) 5029);
        Model lit = tree.light(64, 768, -50, -10, -50);
        writeModel("object-1277-model-1570-lit", (fx) lit, OriginalCapture.map("object_id", 1277, "model_id", 1570,
            "recolor", new int[]{3470, 5029}, "lighting", new int[]{64, 768, -50, -10, -50},
            "matches_fixture", "assets/reference/osrs240/models/tree-1277-yaw-*.png"));
        System.out.println("MODEL tree exported");
    }

    /** Reads a raw obfuscated int field whose name is a Java keyword. */
    static int npcInt(pl definition, String field) throws Exception
    {
        java.lang.reflect.Field member = pl.class.getDeclaredField(field);
        member.setAccessible(true);
        return member.getInt(definition);
    }

    void exportNpcs() throws Exception
    {
        List<Object> npcs = new ArrayList<>();
        int[][] required = {{3028, 6181, 6180}, {2063, 5668, 5666}};
        for (int[] entry : required)
        {
            int npcId = entry[0];
            pl definition = (pl) runtime.game.getNpcDefinition(npcId);
            er data = definition.ax(definition.cn, null, 795096365);
            if (data == null) throw new IllegalStateException("NPC model data missing " + npcId);
            int ambient = 423192979 * definition.dv + 64;
            int contrast = 850 + 2098640321 * npcInt(definition, "do");
            fx base = data.ba(ambient, contrast, -30, -50, -30);
            String baseName = "npc-" + npcId + "-base";
            writeModel(baseName, base, OriginalCapture.map("npc_id", npcId, "models", definition.getModels(),
                "ambient_plus_64", ambient, "contrast_plus_850", contrast,
                "light_direction", new int[]{-30, -50, -30}, "width_scale", definition.getWidthScale(), "height_scale", definition.getHeightScale(),
                "note", "Lit before animation and before source scale; scale applies after animation as in pl.ag"));
            Map<String, Object> sequences = new LinkedHashMap<>();
            for (int i = 1; i < entry.length; i++)
            {
                int sequenceId = entry[i];
                ou sequence = (ou) runtime.game.loadAnimation(sequenceId);
                if (sequence == null) throw new IllegalStateException("Missing native animation " + sequenceId);
                net.runelite.api.Animation animation = sequence;
                int frames = animation.getNumFrames();
                List<String> frameFiles = new ArrayList<>();
                for (int frame = 0; frame < frames; frame++)
                {
                    fx model = definition.ag(sequence, frame, null, -1, null, -520150610);
                    String name = "npc-" + npcId + "-seq-" + sequenceId + "-frame-" + frame;
                    writeModel("baked/" + name, model, OriginalCapture.map("npc_id", npcId, "sequence_id", sequenceId, "frame", frame,
                        "classification", "Native original frame bake for validation of the Rust animation port; source scale already applied"));
                    frameFiles.add("models/baked/" + name + ".bin");
                }
                sequences.put(String.valueOf(sequenceId), OriginalCapture.map("frame_count", frames,
                    "frame_lengths_client_cycles", animation.getFrameLengths(), "baked_frames", frameFiles));
            }
            npcs.add(OriginalCapture.map("npc_id", npcId, "name", definition.getName(), "base_model", "models/" + baseName + ".bin",
                "width_scale", definition.getWidthScale(), "height_scale", definition.getHeightScale(), "sequences", sequences));
        }
        manifest.put("npcs", npcs);
        System.out.println("NPCS exported=" + npcs.size());
    }
}
