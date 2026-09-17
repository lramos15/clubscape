import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.math.BigInteger;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeSet;

/**
 * Exports the original skeletal animation inputs the renderer's Rust `fx.dn`/`fx.rx` port
 * consumes: sequences (frame references, frame lengths, loop fields, hand-item overrides) with
 * every referenced frame's transform list (type, skeleton labels, dx, dy, dz), the M1 NPC
 * definitions (lit unscaled base model, definition sequences, size, scale) and the player
 * building blocks (default kit body model, equippable item models, weapon stance params).
 * Nothing here renders; the buffers are neutral reads of the original structures.
 */
final class AnimExport
{
    final RenderExport export;
    final Map<Integer, Map<String, Object>> sequenceRecords = new LinkedHashMap<>();

    AnimExport(RenderExport export) { this.export = export; }

    static int decode(int field, int encoder)
    {
        int inverse = BigInteger.valueOf(Integer.toUnsignedLong(encoder)).modInverse(BigInteger.ONE.shiftLeft(32)).intValue();
        return field * inverse;
    }

    static Object raw(Object instance, Class<?> owner, String name) throws Exception
    {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.get(instance);
    }

    static int rawInt(Object instance, Class<?> owner, String name) throws Exception
    {
        Field field = owner.getDeclaredField(name);
        field.setAccessible(true);
        return field.getInt(instance);
    }

    /** Default M1 sequence set: player actions bound by the frozen research plus NPC combat sets. */
    static final int[] PLAYER_SEQUENCES = {
        808, 819, 824, 820, 821, 822, 823, // unarmed stand, walk, run, walk-back, shuffle left/right, turn (server appearance defaults)
        836, // death
        829, 12526, // eat
        827, // bury bones
        625, 879, 621, // mining, woodcutting, net fishing
        733, 897, 896, 899, 898, // firemaking, cooking (fire, range), smelting, smithing
        386, 390, 422, 423, // melee stab, slash, punch, kick
        426, // shortbow
        711, // wind strike cast
        // Confirmed additional M1 actor sequences (research/interface-contracts/
        // animation-requirements.json, source metadata animation-authority/inputs.json):
        2305, // milking (native hand overrides left 6244 / right 2437)
        4847, 4850, 4853, 4855, 4857, // ordinary Home Teleport phases (4847 right hand 10214)
        // Qualified legal combat styles: axe hack, blunt spike/pound, spear spike/lunge, sweep.
        395, 400, 401, 428, 429, 440,
    };

    /** Binds the config/model archives the kit, item and param loaders read (client init, gj.az). */
    void bindArchives() throws Exception
    {
        for (Object[] binding : new Object[][] {{pn.class, "ax", 2}, {lp.class, "aw", 2}, {ox.class, "ak", 7}})
        {
            Field field = ((Class<?>) binding[0]).getDeclaredField((String) binding[1]);
            field.setAccessible(true);
            if (field.get(null) == null) field.set(null, export.cache.archive((Integer) binding[2]));
        }
    }

    void run(String[] args) throws Exception
    {
        bindArchives();
        TreeSet<Integer> sequences = new TreeSet<>();
        for (int id : PLAYER_SEQUENCES) sequences.add(id);
        List<Object> npcRecords = new ArrayList<>();
        for (int npcId : npcIds())
        {
            Map<String, Object> record = exportNpc(npcId, sequences);
            npcRecords.add(record);
        }
        export.manifest.put("npc_definitions", npcRecords);
        List<Object> itemRecords = new ArrayList<>();
        TreeSet<Integer> exportedItems = new TreeSet<>();
        for (int itemId : new int[] {882, 1351, 877, 1009, 1949, 1205, 1265, 1173, 1171, 841, 1237, 1277})
        {
            itemRecords.add(exportItem(itemId, sequences));
            exportedItems.add(itemId);
        }
        List<Object> sequenceRecordsOut = new ArrayList<>();
        for (int id : sequences) sequenceRecordsOut.add(exportSequence(id));
        export.manifest.put("sequences", sequenceRecordsOut);
        // The original hand-item override (lc.bd): a sequence with leftHandItem / rightHandItem
        // >= 0 replaces the shield (slot 5) / weapon (slot 3) equipment id with
        // `value - 512 + 2048`; >= 2048 is item `value - 512`, 256..2047 is kit `id - 256`.
        // Every item such a required sequence shows is exported like the worn items; every kit
        // is checked against the cache (value 0 maps to kit 1280, which does not exist -> the
        // slot draws nothing).
        int[] kitFiles = export.cache.archive(2).getFileIds(3);
        TreeSet<Integer> kitIds = new TreeSet<>();
        for (int id : kitFiles) kitIds.add(id);
        Map<String, Object> overrides = new LinkedHashMap<>();
        overrides.put("rule", "lc.bd: sequence.leftHandItem >= 0 -> equipment[5] = value - 512 + 2048; rightHandItem >= 0 -> equipment[3] = value - 512 + 2048; ids >= 2048 are items (id - 2048), 256..2047 kits (id - 256), others nothing");
        overrides.put("source", "ou.by (opcode 6) / ou.bq (opcode 7) decoded per required sequence; lc.at conversion; lc.lk / lc.ak range checks; kit table = archive 2 group 3 file ids");
        List<Object> values = new ArrayList<>();
        TreeSet<Integer> seen = new TreeSet<>();
        for (Object recordObj : sequenceRecordsOut)
        {
            @SuppressWarnings("unchecked") Map<String, Object> record = (Map<String, Object>) recordObj;
            for (String key : new String[] {"left_hand_item", "right_hand_item"})
            {
                int value = ((Number) record.get(key)).intValue();
                if (value < 0 || !seen.add(value)) continue;
                int equipment = value - 512 + 2048;
                Map<String, Object> entry = new LinkedHashMap<>();
                entry.put("value", value);
                entry.put("equipment_id", equipment);
                if (equipment >= 2048)
                {
                    int itemId = equipment - 2048;
                    entry.put("kind", "item");
                    entry.put("item_id", itemId);
                    if (exportedItems.add(itemId))
                    {
                        Map<String, Object> itemRecord = exportItem(itemId, sequences);
                        itemRecord.put("role", "sequence_hand_item");
                        itemRecords.add(itemRecord);
                    }
                }
                else if (equipment >= 256)
                {
                    int kitId = equipment - 256;
                    entry.put("kind", "kit");
                    entry.put("kit_id", kitId);
                    entry.put("kit_exists", kitIds.contains(kitId));
                    entry.put("draws", kitIds.contains(kitId) ? "kit model (unsupported on the penguin body)" : "nothing: no such kit file");
                }
                else
                {
                    entry.put("kind", "none");
                }
                values.add(entry);
            }
        }
        overrides.put("values", values);
        overrides.put("kit_count", kitIds.size());
        export.manifest.put("sequence_hand_overrides", overrides);
        export.manifest.put("equipment_items", itemRecords);
        export.manifest.put("player_reference", exportPlayerReference());
        System.out.println("ANIM sequences=" + sequences.size() + " npcs=" + npcRecords.size() + " items=" + itemRecords.size());
    }

    /** The 26 M1 NPC definitions retained by the content pack. */
    static int[] npcIds()
    {
        return new int[] {306, 311, 732, 2063, 2804, 2813, 2814, 3028, 3305, 3307, 3308, 3309, 3310, 3311, 3312, 3313, 3316, 3317, 3318, 3319,
            4626, 4627, 4628, 7941, 8503, 9244, 9855};
    }

    Map<String, Object> exportNpc(int npcId, TreeSet<Integer> sequences) throws Exception
    {
        pl definition = (pl) export.runtime.game.getNpcDefinition(npcId);
        if (definition == null) throw new IllegalStateException("Missing NPC definition " + npcId);
        pl resolved = definition;
        String morph = null;
        boolean unanimated = decode(rawInt(definition, pl.class, "ch"), 1220125627) == -1;
        if ((definition.cn == null || unanimated) && raw(definition, pl.class, "dl") != null)
        {
            // Varbit/varp morph parent without its own models: the zero-state child is the
            // definition the original shows for a fresh account.
            Method ac = pl.class.getDeclaredMethod("ac", int.class);
            ac.setAccessible(true);
            resolved = (pl) ac.invoke(definition, 652890385);
            if (resolved == null) throw new IllegalStateException("NPC " + npcId + " morphs to nothing in the zero varp state");
            morph = "zero-state child " + resolved.getId();
        }
        er data = resolved.ax(resolved.cn, null, 795096365);
        if (data == null) throw new IllegalStateException("NPC model data missing " + npcId);
        int ambient = 423192979 * rawInt(resolved, pl.class, "dv") + 64;
        int contrast = 850 + 2098640321 * rawInt(resolved, pl.class, "do");
        fx base = data.ba(ambient, contrast, -30, -50, -30);
        String baseName = "npc-" + npcId + "-base";
        if (npcId != 3028 && npcId != 2063)
        {
            export.writeModel(baseName, base, OriginalCapture.map("npc_id", npcId, "models", resolved.getModels(),
                "ambient_plus_64", ambient, "contrast_plus_850", contrast, "light_direction", new int[] {-30, -50, -30},
                "width_scale", resolved.getWidthScale(), "height_scale", resolved.getHeightScale(),
                "note", "Lit before animation and before source scale; scale applies after animation as in pl.ag"));
        }
        Map<String, Object> anims = new LinkedHashMap<>();
        anims.put("stand", decode(rawInt(resolved, pl.class, "ch"), 1220125627));
        anims.put("walk", decode(rawInt(resolved, pl.class, "cw"), -804691593));
        anims.put("rotate_180", decode(rawInt(resolved, pl.class, "cj"), -2105781777));
        anims.put("rotate_left", decode(rawInt(resolved, pl.class, "ci"), -1432063099));
        anims.put("rotate_right", decode(rawInt(resolved, pl.class, "cx"), 852782415));
        anims.put("idle_rotate_left", decode(rawInt(resolved, pl.class, "cz"), 674030075));
        anims.put("idle_rotate_right", decode(rawInt(resolved, pl.class, "cg"), 1213713169));
        anims.put("run", decode(rawInt(resolved, pl.class, "ce"), -1755050233));
        for (Object value : anims.values()) if ((Integer) value >= 0) sequences.add((Integer) value);
        int[] combat = combatSequences(npcId);
        for (int id : combat) sequences.add(id);
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("npc_id", npcId);
        record.put("name", resolved.getName());
        record.put("base_model", "models/" + baseName + ".bin");
        record.put("size", resolved.getSize());
        record.put("width_scale", resolved.getWidthScale());
        record.put("height_scale", resolved.getHeightScale());
        record.put("sequences", anims);
        record.put("combat_sequences", combat);
        if (morph != null) record.put("morph", morph);
        if (raw(resolved, pl.class, "dl") != null) record.put("morph_children", raw(resolved, pl.class, "dl"));
        System.out.println("NPCDEF " + npcId + " " + resolved.getName() + " stand=" + anims.get("stand") + " walk=" + anims.get("walk"));
        return record;
    }

    /** Combat sequences bound by research/reference-pack/v1 audio bindings (server-triggered). */
    static int[] combatSequences(int npcId)
    {
        if (npcId == 3028) return new int[] {6182, 6183, 6184, 6185};
        if (npcId == 3313) return new int[] {4933, 4934, 4935};
        return new int[0];
    }

    Map<String, Object> exportItem(int itemId, TreeSet<Integer> sequences) throws Exception
    {
        op item = (op) export.runtime.game.getItemDefinition(itemId);
        if (item == null) throw new IllegalStateException("Missing item definition " + itemId);
        Method jm = op.class.getDeclaredMethod("jm", op.class, int.class, pi.class, int.class);
        jm.setAccessible(true);
        er male = (er) jm.invoke(null, item, 0, null, 1986500445);
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("item_id", itemId);
        record.put("name", item.getName());
        if (male != null)
        {
            // Lit exactly like the player composition does (lc.bd: er.ba(64, 850, -30, -50, -30)).
            fx lit = male.ba(64, 850, -30, -50, -30);
            String name = "item-" + itemId + "-equip";
            export.writeModel(name, lit, OriginalCapture.map("item_id", itemId, "gender", 0, "source", "op.jm equipped model, lit as in lc.bd"));
            record.put("equip_model", "models/" + name + ".bin");
            record.put("labels", labelSummary(lit));
        }
        else
        {
            record.put("equip_model", null);
        }
        Map<String, Object> params = new LinkedHashMap<>();
        Object table = raw(item, op.class, "ee");
        if (table != null)
        {
            // Iterate the original IterableNodeHashTable (nx) of integer params.
            for (Object node : iterate(table))
            {
                if (node instanceof vg) params.put(String.valueOf(((vg) node).ho), ((vg) node).getValue());
            }
        }
        // Item params in this cache carry only bonuses/speed (keys 0..14, 23, 2431); stance and
        // attack sequences come from the server appearance block, so none are derived here.
        record.put("params", params);
        System.out.println("ITEMDEF " + itemId + " " + item.getName() + " params=" + params);
        return record;
    }

    /** Walks the buckets of the original IterableNodeHashTable (yn): each bucket is a circular list. */
    static List<Object> iterate(Object table) throws Exception
    {
        List<Object> out = new ArrayList<>();
        vq[] buckets = (vq[]) raw(table, yn.class, "af");
        for (vq head : buckets)
        {
            for (vq node = head.hu; node != head; node = node.hu) out.add(node);
        }
        return out;
    }

    /** Per-label vertex centroids and extents of a lit model (for retargeting analysis). */
    static List<Object> labelSummary(fx model)
    {
        List<Object> out = new ArrayList<>();
        if (model.cj == null) return out;
        for (int label = 0; label < model.cj.length; label++)
        {
            int[] group = model.cj[label];
            if (group == null || group.length == 0) continue;
            double sx = 0, sy = 0, sz = 0;
            int minY = Integer.MAX_VALUE, maxY = Integer.MIN_VALUE;
            for (int v : group)
            {
                sx += model.wh[v]; sy += model.pa[v]; sz += model.mn[v];
                minY = Math.min(minY, (int) model.pa[v]); maxY = Math.max(maxY, (int) model.pa[v]);
            }
            out.add(OriginalCapture.map("label", label, "vertices", group.length,
                "centroid", new double[] {sx / group.length, sy / group.length, sz / group.length}, "min_y", minY, "max_y", maxY));
        }
        return out;
    }

    /** Default male player body (kits 0/10/18/26/33/36/42) lit as lc.bd does; retargeting reference only. */
    Map<String, Object> exportPlayerReference() throws Exception
    {
        int[] kits = {0, 10, 18, 26, 33, 36, 42};
        Method kitLookup = hw.class.getDeclaredMethod("az", int.class, int.class);
        kitLookup.setAccessible(true);
        Method bodyModel = of.class.getDeclaredMethod("ax", int.class);
        bodyModel.setAccessible(true);
        List<er> parts = new ArrayList<>();
        List<Object> partNotes = new ArrayList<>();
        for (int kitId : kits)
        {
            of kit = (of) kitLookup.invoke(null, kitId, -1003483374);
            if (kit == null) throw new IllegalStateException("Missing identity kit " + kitId);
            er part = (er) bodyModel.invoke(kit, -597664344);
            if (part == null) throw new IllegalStateException("Identity kit " + kitId + " has no body model");
            parts.add(part);
            partNotes.add(OriginalCapture.map("kit", kitId, "body_part", decode(rawInt(kit, of.class, "bp"), -1237098007)));
        }
        er merged = new er(parts.toArray(new er[0]), parts.size());
        fx lit = merged.ba(64, 850, -30, -50, -30);
        export.writeModel("player-default-male-body", lit, OriginalCapture.map("kits", kits, "source", "lc.bd default male kits merged and lit; retargeting reference, never drawn as the player"));
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("model", "models/player-default-male-body.bin");
        record.put("kits", partNotes);
        record.put("labels", labelSummary(lit));
        record.put("classification", "Human reference for penguin label retargeting; the drawn player body is NPC 2063 model 21547 at 75/128");
        return record;
    }

    Map<String, Object> exportSequence(int sequenceId) throws Exception
    {
        ou sequence = (ou) export.runtime.game.loadAnimation(sequenceId);
        if (sequence == null) throw new IllegalStateException("Missing native animation " + sequenceId);
        int maya = decode(rawInt(sequence, ou.class, "bp"), -139933661);
        if (maya >= 0) throw new IllegalStateException("Sequence " + sequenceId + " is a skeletal (Maya) animation; not supported by the frame port");
        int[] frameRefs = (int[]) raw(sequence, ou.class, "bg");
        int[] lengths = (int[]) raw(sequence, ou.class, "bk");
        int frameStep = decode(rawInt(sequence, ou.class, "bu"), 1039941295);
        int maxLoops = decode(rawInt(sequence, ou.class, "bf"), 1860155365);
        int total = decode(rawInt(sequence, ou.class, "bo"), -293117435);
        // Field encoders from the decompiled ou constructor: by=leftHandItem (opcode 6),
        // bq=rightHandItem (7), bd=priority (10), bl=replyMode (11), bs=precedenceAnimating (9),
        // be=forcedPriority (5), bi=stretches (4, boolean).
        int leftHand = decode(rawInt(sequence, ou.class, "by"), 935967061);
        int rightHand = decode(rawInt(sequence, ou.class, "bq"), 317621309);
        int priority = decode(rawInt(sequence, ou.class, "bd"), 1539095341);
        int replyMode = decode(rawInt(sequence, ou.class, "bl"), -991329609);
        int precedence = decode(rawInt(sequence, ou.class, "bs"), -871691867);
        int[] interleave = (int[]) raw(sequence, ou.class, "ba");
        boolean stretches = (Boolean) raw(sequence, ou.class, "bi");
        ChunkWriter writer = new ChunkWriter();
        writer.ints("SEQH", sequenceId, frameRefs.length, frameStep, maxLoops, total, leftHand, rightHand, priority, replyMode, precedence, stretches ? 1 : 0);
        writer.ints("SEQL", lengths, lengths.length);
        writer.ints("SEQF", frameRefs, frameRefs.length);
        if (interleave != null) writer.ints("SEQI", interleave, interleave.length);
        // Skeletons (em) deduplicated by identity; frames reference them by index.
        List<em> skeletons = new ArrayList<>();
        List<Integer> skeletonTable = new ArrayList<>();
        List<Integer> frameTable = new ArrayList<>();
        Method frames = client.class.getDeclaredMethod("ah", int.class);
        frames.setAccessible(true);
        for (int ref : frameRefs)
        {
            fs archive = (fs) frames.invoke(export.runtime.game, ref >> 16);
            if (archive == null) throw new IllegalStateException("Frame archive " + (ref >> 16) + " unavailable for sequence " + sequenceId);
            et[] all = (et[]) raw(archive, fs.class, "az");
            int index = ref & 0xFFFF;
            if (index >= all.length || all[index] == null) throw new IllegalStateException("Frame " + index + " missing in archive " + (ref >> 16));
            et frame = all[index];
            em skeleton = (em) raw(frame, et.class, "ag");
            int skeletonIndex = -1;
            for (int i = 0; i < skeletons.size(); i++) if (skeletons.get(i) == skeleton) skeletonIndex = i;
            if (skeletonIndex < 0)
            {
                skeletonIndex = skeletons.size();
                skeletons.add(skeleton);
                int[] types = (int[]) raw(skeleton, em.class, "ax");
                int[][] labels = (int[][]) raw(skeleton, em.class, "ac");
                skeletonTable.add(types.length);
                for (int t = 0; t < types.length; t++)
                {
                    skeletonTable.add(types[t]);
                    skeletonTable.add(labels[t].length);
                    for (int label : labels[t]) skeletonTable.add(label);
                }
            }
            int count = rawInt(frame, et.class, "as");
            int[] transformIndex = (int[]) raw(frame, et.class, "ax");
            int[] dx = (int[]) raw(frame, et.class, "ac");
            int[] dy = (int[]) raw(frame, et.class, "aa");
            int[] dz = (int[]) raw(frame, et.class, "ao");
            boolean alpha = (Boolean) raw(frame, et.class, "al");
            frameTable.add(ref); frameTable.add(skeletonIndex); frameTable.add(count); frameTable.add(alpha ? 1 : 0);
            for (int i = 0; i < count; i++)
            {
                frameTable.add(transformIndex[i]); frameTable.add(dx[i]); frameTable.add(dy[i]); frameTable.add(dz[i]);
            }
        }
        writer.ints("SKEL", SceneExport.toArray(skeletonTable)).ints("FRMT", SceneExport.toArray(frameTable));
        String fileKey = "anim/seq-" + sequenceId + ".bin";
        Path file = export.output.resolve(fileKey);
        Files.createDirectories(file.getParent());
        String sha = writer.write(file);
        Map<String, Object> record = new LinkedHashMap<>();
        record.put("sequence_id", sequenceId);
        record.put("file", fileKey);
        record.put("sha256", sha);
        record.put("frame_count", frameRefs.length);
        record.put("frame_lengths_client_cycles", lengths);
        record.put("frame_step", frameStep);
        record.put("max_loops", maxLoops);
        record.put("left_hand_item", leftHand);
        record.put("right_hand_item", rightHand);
        record.put("priority", priority);
        record.put("skeletons", skeletons.size());
        export.record(fileKey, sha, Files.size(file), record);
        return record;
    }
}
