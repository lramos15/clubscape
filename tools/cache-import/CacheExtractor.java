import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.awt.image.BufferedImage;
import java.io.IOException;
import java.io.OutputStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.ByteBuffer;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.HexFormat;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.zip.GZIPOutputStream;
import javax.imageio.ImageIO;
import javax.sound.midi.MidiSystem;
import net.runelite.cache.IndexType;
import net.runelite.cache.definitions.*;
import net.runelite.cache.definitions.loaders.*;
import net.runelite.cache.definitions.loaders.sound.SoundEffectLoader;
import net.runelite.cache.fs.*;
import net.runelite.cache.region.Region;
import net.runelite.cache.region.RegionLoader;

/**
 * A lossless source-data boundary, not a ClubScape or stock-game renderer.
 * Decoders are supplied by the pinned RuneLite cache library (see dependencies.json).
 */
public final class CacheExtractor implements AutoCloseable
{
    private static final Gson GSON = new GsonBuilder().disableHtmlEscaping().serializeNulls().create();
    private final Store store;
    private final Path output;
    private final int revision;
    private final int cacheId;
    private final Map<String, ArchiveFiles> loaded = new LinkedHashMap<>();
    private final Map<String, Map<String, Object>> groupSources = new LinkedHashMap<>();
    private final List<Map<String, Object>> records = new ArrayList<>();
    private final Map<String, Integer> counts = new TreeMap<>();
    private final Map<Integer, ObjectDefinition> objects = new TreeMap<>();
    private final Map<Integer, NpcDefinition> npcs = new TreeMap<>();
    private final Map<Integer, SequenceDefinition> sequences = new TreeMap<>();
    private final Map<Integer, FramemapDefinition> skeletons = new TreeMap<>();
    private final Set<Integer> modelIds = new TreeSet<>();
    private final Set<Integer> sequenceIds = new TreeSet<>();
    private final Set<Integer> spriteIds = new TreeSet<>();
    private final Set<Integer> fontIds = new TreeSet<>();
    private final Set<Integer> textureIds = new TreeSet<>();
    private final Set<Integer> soundIds = new TreeSet<>();
    private final List<Integer> absentMapSquares = new ArrayList<>();
    private final List<Integer> cachedAnimationIds = new ArrayList<>();

    private CacheExtractor(Path cache, Path output, int revision, int cacheId) throws IOException
    {
        if (!Files.isRegularFile(cache.resolve("main_file_cache.dat2"))
            || !Files.isRegularFile(cache.resolve("main_file_cache.idx255")))
        {
            throw new IOException("Missing complete cache: " + cache);
        }
        this.output = output;
        this.revision = revision;
        this.cacheId = cacheId;
        store = new Store(cache.toFile());
        store.load();
    }

    private static Map<String, Object> map(Object... entries)
    {
        Map<String, Object> result = new LinkedHashMap<>();
        for (int i = 0; i < entries.length; i += 2)
        {
            result.put((String) entries[i], entries[i + 1]);
        }
        return result;
    }

    private static String hash(byte[] data) throws Exception
    {
        return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(data));
    }

    private ArchiveFiles group(int indexId, int groupId) throws Exception
    {
        String key = indexId + "/" + groupId;
        if (loaded.containsKey(key)) return loaded.get(key);
        Index index = store.findIndex(indexId);
        if (index == null) throw new IOException("Missing index " + indexId);
        Archive archive = index.getArchive(groupId);
        if (archive == null) throw new IOException("Missing archive " + key);
        byte[] data = store.getStorage().loadArchive(archive);
        if (data == null || data.length == 0) throw new IOException("Empty archive " + key);
        int expectedRevision = archive.getRevision();
        int compressedLength = ByteBuffer.wrap(data, 1, 4).getInt();
        long containerLength = 5L + compressedLength + (data[0] == 0 ? 0 : 4);
        if (compressedLength < 0 || containerLength + 2 != data.length
            || (ByteBuffer.wrap(data, data.length - 2, 2).getShort() & 65535) != (expectedRevision & 65535))
        {
            throw new IOException("Invalid container length or version trailer: " + key);
        }
        // Disk trailers keep only the low 16 bits; preserve the index's full 32-bit revision.
        ArchiveFiles files = archive.getFiles(Arrays.copyOf(data, (int) containerLength));
        loaded.put(key, files);
        groupSources.put(key, map("archive", indexId, "group", groupId,
            "crc32", Integer.toUnsignedLong(archive.getCrc()), "revision", expectedRevision,
            "container_size_bytes", data.length, "container_sha256", hash(data),
            "container_includes_version_trailer", true));
        return files;
    }

    private byte[] bytes(int index, int archive, int file) throws Exception
    {
        FSFile input = group(index, archive).findFile(file);
        if (input == null || input.getContents() == null || input.getContents().length == 0)
        {
            throw new IOException("Missing/empty source file " + index + "/" + archive + "/" + file);
        }
        return input.getContents();
    }

    private Map<String, Object> source(int index, int archive, int file) throws Exception
    {
        byte[] data = bytes(index, archive, file);
        return map("group_key", index + "/" + archive, "file", file,
            "size_bytes", data.length, "sha256", hash(data));
    }

    private Map<String, Object> write(String relative, byte[] data) throws Exception
    {
        Path path = output.resolve(relative);
        Files.createDirectories(path.getParent());
        Files.write(path, data);
        return map("path", relative, "size_bytes", data.length, "sha256", hash(data));
    }

    private Map<String, Object> json(String relative, Object value) throws Exception
    {
        byte[] data = GSON.toJson(value).getBytes(StandardCharsets.UTF_8);
        if (relative.endsWith(".gz"))
        {
            var buffer = new java.io.ByteArrayOutputStream();
            try (OutputStream gzip = new GZIPOutputStream(buffer)) { gzip.write(data); }
            Map<String, Object> result = write(relative, buffer.toByteArray());
            result.put("json_size_bytes", data.length);
            result.put("json_sha256", hash(data));
            return result;
        }
        return write(relative, data);
    }

    private Map<String, Object> record(String kind, String id, List<Map<String, Object>> sources,
                                       Object statistics)
    {
        Map<String, Object> result = map("asset_id", "asset.source.osrs.cache" + cacheId + "." + kind + "." + id,
            "kind", kind, "source", sources, "statistics", statistics, "outputs", new ArrayList<>());
        records.add(result);
        counts.merge(kind, 1, Integer::sum);
        return result;
    }

    @SuppressWarnings("unchecked")
    private void add(Map<String, Object> record, Map<String, Object> file)
    {
        ((List<Map<String, Object>>) record.get("outputs")).add(file);
    }

    private void raw(Map<String, Object> record, int index, int archive, int file) throws Exception
    {
        add(record, write("raw/" + index + "/" + archive + "/" + file + ".bin",
            bytes(index, archive, file)));
    }

    private void definition(String kind, int id, int group, Object value) throws Exception
    {
        var record = record(kind, "" + id, List.of(source(2, group, id)), null);
        add(record, json("definitions/" + kind + "/" + id + ".json", value));
        raw(record, 2, group, id);
    }

    private void readEntities() throws Exception
    {
        ObjectLoader objectLoader = new ObjectLoader().configureForRevision(store.findIndex(2).getArchive(6).getRevision());
        for (FSFile f : group(2, 6).getFiles())
        {
            objects.put(f.getFileId(), objectLoader.load(f.getFileId(), f.getContents()));
        }
        NpcLoader npcLoader = new NpcLoader().configureForRevision(store.findIndex(2).getArchive(9).getRevision());
        for (FSFile f : group(2, 9).getFiles())
        {
            npcs.put(f.getFileId(), npcLoader.load(f.getFileId(), f.getContents()));
        }
    }

    private void scan() throws Exception
    {
        readEntities();
        List<Object> indexSummary = new ArrayList<>();
        for (Index i : store.getIndexes())
        {
            indexSummary.add(map("id", i.getId(), "groups", i.getArchives().size(),
                "revision", i.getRevision(), "named", i.isNamed()));
        }
        List<Object> candidates = new ArrayList<>();
        for (NpcDefinition npc : npcs.values())
        {
            if ((npc.name.equals("Goblin") || npc.name.equals("Penguin"))
                && npc.models != null && candidates.size() < 100)
            {
                candidates.add(map("kind", "npc", "id", npc.id, "name", npc.name,
                    "models", npc.models, "standing", npc.standingAnimation,
                    "walking", npc.walkingAnimation, "combat_level", npc.combatLevel));
            }
        }
        for (ObjectDefinition object : objects.values())
        {
            if (object.getName().equals("Tree") && object.getObjectModels() != null
                && candidates.size() < 125)
            {
                candidates.add(map("kind", "object", "id", object.getId(), "name", object.getName(),
                    "models", object.getObjectModels(), "types", object.getObjectTypes()));
            }
        }
        List<Object> names = new ArrayList<>();
        for (String name : new String[]{"p11_full", "p12_full", "b12_full", "titlebox",
            "titlebutton", "mapscene", "mapfunction", "sideicons", "scrollbar",
            "scape main", "newbie melody", "harmony", "autumn voyage", "scape cave"})
        {
            for (int index : new int[]{6, 8, 13})
            {
                Archive a = store.findIndex(index).findArchiveByName(name);
                if (a != null) names.add(map("name", name, "index", index, "group", a.getArchiveId()));
            }
        }
        List<Object> journeyNpcs = new ArrayList<>();
        Set<String> namesNeeded = Set.of("Gielinor Guide", "RuneScape Guide", "Survival Expert",
            "Master Chef", "Quest Guide", "Mining Instructor", "Combat Instructor", "Banker",
            "Financial Advisor", "Brother Brace", "Magic Instructor", "Cook", "Millie Miller",
            "Fred the Farmer", "Shop keeper", "Shop assistant", "Cow", "Cow calf", "Chicken",
            "Rat", "Giant rat");
        for (NpcDefinition npc : npcs.values())
        {
            if (namesNeeded.contains(npc.name))
            {
                journeyNpcs.add(map("id", npc.id, "name", npc.name, "models", npc.models,
                    "standing", npc.standingAnimation, "walking", npc.walkingAnimation));
            }
        }
        List<Object> binary = new ArrayList<>();
        for (Archive a : store.findIndex(10).getArchives())
        {
            binary.add(map("id", a.getArchiveId(), "name_hash", a.getNameHash()));
        }
        json("scan.json", map("indexes", indexSummary, "candidates", candidates, "named_inputs", names,
            "journey_npcs", journeyNpcs, "binary", binary));
        json("npc-catalogue.json", npcs);
        Set<Integer> deathCoffers = new TreeSet<>();
        for (ObjectDefinition object : objects.values())
        {
            if (object.getName().equalsIgnoreCase("Death's coffer")) deathCoffers.add(object.getId());
        }
        List<Object> dependencyLocations = new ArrayList<>();
        List<Object> unparsedExploratoryLocations = new ArrayList<>();
        for (Archive archive : store.findIndex(5).getArchives())
        {
            if (deathCoffers.isEmpty()) break;
            int regionId = archive.getArchiveId();
            FSFile file = group(5, regionId).findFile(1);
            if (file == null) continue;
            LocationsDefinition locs;
            try
            {
                locs = new LocationsLoader().load(regionId >> 8, regionId & 255, file.getContents());
            }
            catch (RuntimeException error)
            {
                // This whole-cache lookup is exploratory. Required extraction regions still fail closed.
                unparsedExploratoryLocations.add(map("region_id", regionId, "size_bytes", file.getContents().length,
                    "sha256", hash(file.getContents()), "error", error.getClass().getSimpleName()));
                continue;
            }
            for (var loc : locs.getLocations())
            {
                if (deathCoffers.contains(loc.getId()))
                {
                    dependencyLocations.add(map("object_id", loc.getId(), "name", objects.get(loc.getId()).getName(),
                        "region_id", regionId, "world_x", (regionId >> 8) * 64 + loc.getPosition().getX(),
                        "world_y", (regionId & 255) * 64 + loc.getPosition().getY(),
                        "plane", loc.getPosition().getZ(), "type", loc.getType(),
                        "source", source(5, regionId, 1)));
                }
            }
        }
        json("dependency-locations.json", map("derived_object_ids", deathCoffers, "placements", dependencyLocations,
            "unparsed_exploratory_location_files", unparsedExploratoryLocations,
            "note", "Source locations, not live instance-copy mapping or an approved camera."));
        System.out.println("Scanned " + objects.size() + " objects and " + npcs.size() + " NPCs; " + output.resolve("scan.json"));
    }

    private static void include(Set<Integer> ids, int[] values)
    {
        if (values != null) for (int value : values) if (value >= 0) ids.add(value);
    }

    private void npc(int id) throws Exception
    {
        NpcDefinition npc = npcs.get(id);
        if (npc == null) throw new IOException("Missing NPC definition " + id);
        definition("npc", id, 9, npc);
        include(modelIds, npc.models);
        include(modelIds, npc.chatheadModels);
        for (Field field : NpcDefinition.class.getFields())
        {
            if (field.getName().endsWith("Animation"))
            {
                int animation = field.getInt(npc);
                if (animation >= 0) sequenceIds.add(animation);
            }
        }
    }

    private void terrain(int regionId, Set<Integer> objectIds, Set<Integer> underlays,
                         Set<Integer> overlays) throws Exception
    {
        if (store.findIndex(5).getArchive(regionId) == null)
        {
            absentMapSquares.add(regionId);
            return;
        }
        int rx = regionId >> 8, ry = regionId & 255;
        byte[] terrain = bytes(5, regionId, 0), locations = bytes(5, regionId, 1);
        MapDefinition map = new MapLoader().load(rx, ry, terrain);
        LocationsDefinition locs = new LocationsLoader().load(rx, ry, locations);
        Region region = new RegionLoader(store, ignored -> null).loadRegion(regionId, map, locs);
        int[][] fields = new int[7][4 * 64 * 64];
        int pos = 0, minimumHeight = Integer.MAX_VALUE, maximumHeight = Integer.MIN_VALUE;
        for (int plane = 0; plane < 4; plane++)
        {
            for (int x = 0; x < 64; x++)
            {
                for (int y = 0; y < 64; y++, pos++)
                {
                    int height = region.getTileHeight(plane, x, y);
                    fields[0][pos] = height;
                    fields[1][pos] = region.getUnderlayId(plane, x, y);
                    fields[2][pos] = region.getOverlayId(plane, x, y);
                    fields[3][pos] = region.getOverlayPath(plane, x, y);
                    fields[4][pos] = region.getOverlayRotation(plane, x, y);
                    fields[5][pos] = region.getTileSetting(plane, x, y) & 255;
                    fields[6][pos] = map.getTiles()[plane][x][y].getHeight() == null
                        ? -1 : map.getTiles()[plane][x][y].getHeight();
                    if (fields[1][pos] > 0) underlays.add(fields[1][pos] - 1);
                    if (fields[2][pos] > 0) overlays.add(fields[2][pos] - 1);
                    minimumHeight = Math.min(minimumHeight, height);
                    maximumHeight = Math.max(maximumHeight, height);
                }
            }
        }
        List<Object> placements = new ArrayList<>();
        for (var loc : region.getLocations())
        {
            var p = loc.getPosition();
            if (p.getX() < rx * 64 || p.getX() >= rx * 64 + 64
                || p.getY() < ry * 64 || p.getY() >= ry * 64 + 64
                || p.getZ() < 0 || p.getZ() > 3)
            {
                throw new IOException("Out-of-bounds source placement in " + regionId);
            }
            placements.add(new int[]{loc.getId(), p.getX(), p.getY(), p.getZ(), loc.getType(), loc.getOrientation()});
            objectIds.add(loc.getId());
        }
        var record = record("region", "" + regionId,
            List.of(source(5, regionId, 0), source(5, regionId, 1)),
            map("tiles", pos, "placements", placements.size(), "height_min", minimumHeight, "height_max", maximumHeight));
        add(record, json("world/" + regionId + ".json.gz", map("schema_version", 1,
            "region_id", regionId, "base_x", rx * 64, "base_y", ry * 64,
            "dimensions", new int[]{4, 64, 64}, "array_order", "(plane * 64 + local_x) * 64 + local_y",
            "heights", fields[0], "underlay_ids", fields[1], "overlay_ids", fields[2],
            "overlay_shapes", fields[3], "overlay_rotations", fields[4], "tile_settings", fields[5],
            "encoded_heights", fields[6],
            "placement_columns", new String[]{"object_id", "world_x", "world_y", "plane", "type", "orientation"},
            "placements", placements)));
        raw(record, 5, regionId, 0);
        raw(record, 5, regionId, 1);
        counts.merge("placed_objects", placements.size(), Integer::sum);
    }

    private void objectClosure(Set<Integer> ids) throws Exception
    {
        Set<Integer> done = new TreeSet<>();
        while (!done.containsAll(ids))
        {
            for (int id : new TreeSet<>(ids))
            {
                if (!done.add(id)) continue;
                ObjectDefinition object = objects.get(id);
                if (object == null) throw new IOException("Missing placed object " + id);
                definition("object", id, 6, object);
                include(ids, object.getConfigChangeDest());
                include(modelIds, object.getObjectModels());
                if (object.getAnimationID() >= 0) sequenceIds.add(object.getAnimationID());
                if (object.getAmbientSoundId() >= 0) soundIds.add(object.getAmbientSoundId());
                include(soundIds, object.getAmbientSoundIds());
                if (object.getMapSceneID() >= 0)
                {
                    Archive scene = store.findIndex(8).findArchiveByName("mapscene");
                    if (scene != null) spriteIds.add(scene.getArchiveId());
                }
            }
        }
    }

    private void model(int id) throws Exception
    {
        byte[] data = bytes(7, id, 0);
        ModelDefinition model = new ModelLoader().load(id, data);
        if (model.vertexCount < 0 || model.faceCount < 0 || (model.vertexCount == 0 && model.faceCount > 0))
        {
            throw new IOException("Invalid model counts " + id);
        }
        int[] minimum = {Integer.MAX_VALUE, Integer.MAX_VALUE, Integer.MAX_VALUE};
        int[] maximum = {Integer.MIN_VALUE, Integer.MIN_VALUE, Integer.MIN_VALUE};
        int[][] axes = {model.vertexX, model.vertexY, model.vertexZ};
        for (int axis = 0; axis < 3; axis++)
        {
            if (axes[axis].length != model.vertexCount) throw new IOException("Bad vertices: " + id);
            if (model.vertexCount == 0) minimum[axis] = maximum[axis] = 0;
            for (int coordinate : axes[axis])
            {
                minimum[axis] = Math.min(minimum[axis], coordinate);
                maximum[axis] = Math.max(maximum[axis], coordinate);
                if (Math.abs((long) coordinate) > 1_000_000) throw new IOException("Implausible model bounds: " + id);
            }
        }
        for (int[] indices : new int[][]{model.faceIndices1, model.faceIndices2, model.faceIndices3})
        {
            if (indices.length != model.faceCount) throw new IOException("Bad face array: " + id);
            for (int vertex : indices)
            {
                if (vertex < 0 || vertex >= model.vertexCount) throw new IOException("Bad face index: " + id);
            }
        }
        if (model.faceTextures != null)
        {
            for (short texture : model.faceTextures) if (texture != -1) textureIds.add(texture & 65535);
        }
        model.computeNormals();
        model.computeTextureUVCoordinates();
        var record = record("model", "" + id, List.of(source(7, id, 0)),
            map("vertices", model.vertexCount, "faces", model.faceCount, "bounds_min", minimum, "bounds_max", maximum,
                "source_empty_mesh", model.vertexCount == 0 || model.faceCount == 0));
        add(record, json("models/" + id + ".json.gz", map("schema_version", 1, "model", model,
            "vertex_normals", model.vertexNormals, "face_normals", model.faceNormals,
            "face_texture_u", model.faceTextureUCoordinates, "face_texture_v", model.faceTextureVCoordinates)));
        raw(record, 7, id, 0);
    }

    private void interfaces(int groupId) throws Exception
    {
        InterfaceLoader loader = new InterfaceLoader().configureForRevision(store.findIndex(3).getRevision());
        List<Object> widgets = new ArrayList<>();
        List<Map<String, Object>> sources = new ArrayList<>();
        for (FSFile f : group(3, groupId).getFiles())
        {
            InterfaceDefinition widget = loader.load(groupId << 16 | f.getFileId(), f.getContents());
            widgets.add(widget);
            sources.add(source(3, groupId, f.getFileId()));
            if (widget.spriteId >= 0) spriteIds.add(widget.spriteId);
            if (widget.alternateSpriteId >= 0) spriteIds.add(widget.alternateSpriteId);
            include(spriteIds, widget.sprites);
            if (widget.fontId >= 0) fontIds.add(widget.fontId);
            if (widget.modelType == 1 && widget.modelId >= 0) modelIds.add(widget.modelId);
            if (widget.animation >= 0) sequenceIds.add(widget.animation);
            if (widget.alternateAnimation >= 0) sequenceIds.add(widget.alternateAnimation);
        }
        var record = record("interface", "" + groupId, sources, map("widgets", widgets.size()));
        add(record, json("interfaces/" + groupId + ".json.gz", widgets));
        for (FSFile f : group(3, groupId).getFiles()) raw(record, 3, groupId, f.getFileId());
    }

    private void sprite(int id) throws Exception
    {
        SpriteDefinition[] sprites = new SpriteLoader().load(id, bytes(8, id, 0));
        if (sprites.length == 0) throw new IOException("No sprites: " + id);
        int cellWidth = 1, cellHeight = 1;
        long nontransparentPixels = 0;
        for (SpriteDefinition sprite : sprites)
        {
            if (sprite.getWidth() < 0 || sprite.getHeight() < 0
                || sprite.getPixels().length != sprite.getWidth() * sprite.getHeight()
                || sprite.getOffsetX() < 0 || sprite.getOffsetY() < 0
                || sprite.getOffsetX() + sprite.getWidth() > sprite.getMaxWidth()
                || sprite.getOffsetY() + sprite.getHeight() > sprite.getMaxHeight())
            {
                throw new IOException("Invalid sprite dimensions: " + id);
            }
            cellWidth = Math.max(cellWidth, sprite.getMaxWidth());
            cellHeight = Math.max(cellHeight, sprite.getMaxHeight());
            for (int pixel : sprite.getPixels()) if ((pixel >>> 24) != 0) nontransparentPixels++;
        }
        int columns = Math.min(16, sprites.length);
        int width = Math.multiplyExact(columns, cellWidth);
        int height = Math.multiplyExact((sprites.length + columns - 1) / columns, cellHeight);
        if ((long) width * height > 64_000_000) throw new IOException("Oversized atlas: " + id);
        BufferedImage atlas = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
        List<Object> frames = new ArrayList<>();
        for (int i = 0; i < sprites.length; i++)
        {
            SpriteDefinition s = sprites[i];
            int x = (i % columns) * cellWidth + s.getOffsetX();
            int y = (i / columns) * cellHeight + s.getOffsetY();
            if (s.getWidth() > 0 && s.getHeight() > 0)
            {
                atlas.setRGB(x, y, s.getWidth(), s.getHeight(), s.getPixels(), 0, s.getWidth());
            }
            frames.add(map("frame", s.getFrame(), "atlas_x", x, "atlas_y", y,
                "width", s.getWidth(), "height", s.getHeight(), "offset_x", s.getOffsetX(),
                "offset_y", s.getOffsetY(), "canvas_width", s.getMaxWidth(), "canvas_height", s.getMaxHeight(),
                "palette", s.palette, "pixel_indices_unsigned", unsigned(s.pixelIdx)));
        }
        var png = new java.io.ByteArrayOutputStream();
        if (!ImageIO.write(atlas, "png", png)) throw new IOException("PNG writer unavailable");
        var record = record("sprite", "" + id, List.of(source(8, id, 0)),
            map("frames", sprites.length, "atlas_width", width, "atlas_height", height,
                "nontransparent_pixels", nontransparentPixels));
        add(record, write("sprites/" + id + ".png", png.toByteArray()));
        add(record, json("sprites/" + id + ".json.gz", map("schema_version", 1,
            "pixel_format", "PNG RGBA8, straight alpha, source palette and offsets preserved",
            "frames", frames)));
        raw(record, 8, id, 0);
    }

    private static int[] unsigned(byte[] bytes)
    {
        if (bytes == null) return null;
        int[] result = new int[bytes.length];
        for (int i = 0; i < result.length; i++) result[i] = bytes[i] & 255;
        return result;
    }

    private void font(int id) throws Exception
    {
        byte[] data = bytes(13, id, 0);
        FontDefinition font = new FontLoader().load(data);
        if (font.ascent <= 0 || Arrays.stream(font.advances).sum() == 0)
        {
            throw new IOException("Invalid font metrics " + id);
        }
        spriteIds.add(id);
        var record = record("font", "" + id, List.of(source(13, id, 0)),
            map("glyphs", font.advances.length, "ascent", font.ascent));
        add(record, json("fonts/" + id + ".json", map("schema_version", 1, "advances", font.advances,
            "ascent", font.ascent, "glyph_sprite_group", id, "encoding", "OSRS CP1252 byte indices",
            "kerning", null, "kerning_note", "The original 257-byte metrics format contains no kerning table.")));
        raw(record, 13, id, 0);
    }

    private void animationClosure() throws Exception
    {
        SequenceLoader loader = new SequenceLoader().configureForRevision(store.findIndex(2).getArchive(12).getRevision());
        Set<Integer> frames = new TreeSet<>();
        for (int id : sequenceIds)
        {
            SequenceDefinition sequence = loader.load(id, bytes(2, 12, id));
            sequences.put(id, sequence);
            include(frames, sequence.frameIDs);
            include(frames, sequence.chatFrameIds);
            for (var sound : sequence.frameSounds.values()) soundIds.add(sound.getId());
            JsonObject value = GSON.toJsonTree(sequence).getAsJsonObject();
            value.add("frameSounds", GSON.toJsonTree(sequence.frameSounds.asMap()));
            definition("sequence", id, 12, value);
            if (sequence.animMayaID >= 0) cachedAnimationIds.add(sequence.animMayaID);
        }
        for (int packed : frames)
        {
            int groupId = packed >>> 16, fileId = packed & 65535;
            byte[] data = bytes(0, groupId, fileId);
            int skeletonId = (data[0] & 255) << 8 | data[1] & 255;
            FramemapDefinition skeleton = skeleton(skeletonId);
            FrameDefinition frame = new FrameLoader().load(skeleton, packed, data);
            var record = record("frame", groupId + "." + fileId, List.of(source(0, groupId, fileId)), null);
            add(record, json("animations/frames/" + groupId + "-" + fileId + ".json", frame));
            raw(record, 0, groupId, fileId);
        }
        for (int id : new TreeSet<>(cachedAnimationIds))
        {
            // Modern curve animations live in index 22, not the legacy frame index.
            int index = IndexType.ANIMAYAS.getNumber();
            int groupId = id >>> 16, fileId = id & 65535;
            byte[] data = bytes(index, groupId, fileId);
            int skeletonId = (data[1] & 255) << 8 | data[2] & 255;
            skeleton(skeletonId);
            var record = record("cached-animation", "" + id, List.of(source(index, groupId, fileId)),
                map("format_version", data[0] & 255, "skeleton_id", skeletonId, "curve_decoder", "not_implemented"));
            raw(record, index, groupId, fileId);
        }
    }

    private FramemapDefinition skeleton(int id) throws Exception
    {
        if (skeletons.containsKey(id)) return skeletons.get(id);
        byte[] data = bytes(1, id, 0);
        FramemapDefinition skeleton = new FramemapLoader().load(id, data);
        skeletons.put(id, skeleton);
        var record = record("skeleton", "" + id, List.of(source(1, id, 0)), map("legacy_transform_groups", skeleton.length));
        add(record, json("animations/skeletons/" + id + ".json", skeleton));
        raw(record, 1, id, 0);
        return skeleton;
    }

    private void music(int id, String label) throws Exception
    {
        Archive named = store.findIndex(6).findArchiveByName(label.toLowerCase(Locale.ROOT));
        if (named == null || named.getArchiveId() != id)
        {
            throw new IOException("Music name/hash does not match the selected archive: " + label + "/" + id);
        }
        byte[] data = bytes(6, id, 0);
        TrackDefinition track = new TrackLoader().load(data);
        var sequence = MidiSystem.getSequence(new java.io.ByteArrayInputStream(track.midi));
        if (sequence.getTracks().length == 0 || sequence.getTickLength() == 0)
        {
            throw new IOException("Empty decoded MIDI " + id);
        }
        var record = record("music", "" + id, List.of(source(6, id, 0)),
            map("label", label, "midi_tracks", sequence.getTracks().length, "midi_ticks", sequence.getTickLength(),
                "duration_microseconds", sequence.getMicrosecondLength(), "runtime_audio_verified", false));
        add(record, write("audio/music/" + id + ".mid", track.midi));
        raw(record, 6, id, 0);
    }

    private void sound(int id) throws Exception
    {
        var effect = new SoundEffectLoader().load(bytes(4, id, 0));
        var record = record("sound", "" + id, List.of(source(4, id, 0)), map("synthesis_performed", false));
        add(record, json("audio/sounds/" + id + ".json.gz", effect));
        raw(record, 4, id, 0);
    }

    private void items(JsonArray requested) throws Exception
    {
        Set<Integer> ids = new TreeSet<>(), done = new TreeSet<>();
        for (JsonElement id : requested) ids.add(id.getAsInt());
        ItemLoader loader = new ItemLoader();
        while (!done.containsAll(ids))
        {
            for (int id : new TreeSet<>(ids))
            {
                if (!done.add(id)) continue;
                ItemDefinition item = loader.load(id, bytes(2, 10, id));
                definition("item", id, 10, item);
                include(modelIds, new int[]{item.inventoryModel, item.maleModel0, item.maleModel1,
                    item.maleModel2, item.femaleModel0, item.femaleModel1, item.femaleModel2,
                    item.maleHeadModel, item.maleHeadModel2, item.femaleHeadModel, item.femaleHeadModel2});
                include(ids, new int[]{item.notedID, item.notedTemplate, item.boughtId, item.boughtTemplateId,
                    item.placeholderId, item.placeholderTemplateId});
                if (item.countObj != null)
                {
                    for (int i = 0; i < item.countObj.length; i++)
                    {
                        if (item.countCo[i] > 0) ids.add(item.countObj[i]);
                    }
                }
            }
        }
    }

    private void title() throws Exception
    {
        Archive archive = store.findIndex(10).findArchiveByName("title.jpg");
        if (archive == null) throw new IOException("Missing original title.jpg");
        int id = archive.getArchiveId();
        byte[] data = bytes(10, id, 0);
        BufferedImage image = ImageIO.read(new java.io.ByteArrayInputStream(data));
        if (image == null || image.getWidth() <= 0 || image.getHeight() <= 0)
        {
            throw new IOException("Undecodable original title.jpg");
        }
        var record = record("title", "background", List.of(source(10, id, 0)),
            map("width", image.getWidth(), "height", image.getHeight(), "source_game_capture", false));
        add(record, write("title/title.jpg", data));
        raw(record, 10, id, 0);
    }

    private void musicSynthesisInputs() throws Exception
    {
        for (int index : new int[]{14, 15})
        {
            for (Archive archive : store.findIndex(index).getArchives())
            {
                int group = archive.getArchiveId();
                for (FSFile file : group(index, group).getFiles())
                {
                    int id = file.getFileId();
                    var record = record(index == 14 ? "music-sample" : "music-patch",
                        group + "." + id, List.of(source(index, group, id)),
                        map("decoded_or_played", false, "role", index == 14 && group == 0
                            ? "original Vorbis setup data" : "original music synthesis input"));
                    raw(record, index, group, id);
                }
            }
        }
    }

    private void extract(JsonObject request) throws Exception
    {
        readEntities();
        Set<Integer> objectIds = new TreeSet<>(), underlays = new TreeSet<>(), overlays = new TreeSet<>();
        Set<Integer> regions = new TreeSet<>();
        for (JsonElement rectangle : request.getAsJsonArray("region_rectangles"))
        {
            JsonObject r = rectangle.getAsJsonObject();
            for (int x = r.get("min_x").getAsInt(); x <= r.get("max_x").getAsInt(); x++)
            {
                for (int y = r.get("min_y").getAsInt(); y <= r.get("max_y").getAsInt(); y++) regions.add(x << 8 | y);
            }
        }
        for (int region : regions) terrain(region, objectIds, underlays, overlays);
        for (JsonElement id : request.getAsJsonArray("object_ids")) objectIds.add(id.getAsInt());
        objectClosure(objectIds);
        for (JsonElement id : request.getAsJsonArray("npc_ids")) npc(id.getAsInt());
        items(request.getAsJsonArray("item_ids"));
        for (JsonElement id : request.getAsJsonArray("interface_groups")) interfaces(id.getAsInt());
        for (JsonElement name : request.getAsJsonArray("named_sprites"))
        {
            Archive archive = store.findIndex(8).findArchiveByName(name.getAsString());
            if (archive == null) throw new IOException("Missing named sprite " + name);
            spriteIds.add(archive.getArchiveId());
        }
        for (JsonElement name : request.getAsJsonArray("named_fonts"))
        {
            Archive archive = store.findIndex(13).findArchiveByName(name.getAsString());
            if (archive == null) throw new IOException("Missing named font " + name);
            fontIds.add(archive.getArchiveId());
        }
        for (int id : underlays) definition("underlay", id, 1, new UnderlayLoader().load(id, bytes(2, 1, id)));
        for (int id : overlays)
        {
            OverlayDefinition overlay = new OverlayLoader().load(id, bytes(2, 4, id));
            definition("overlay", id, 4, overlay);
            if (overlay.getTexture() >= 0) textureIds.add(overlay.getTexture());
        }
        for (int id : modelIds) model(id);
        for (int id : textureIds)
        {
            TextureDefinition texture = new TextureLoader().setRev233(true).load(id, bytes(9, 0, id));
            var record = record("texture", "" + id, List.of(source(9, 0, id)), null);
            add(record, json("textures/" + id + ".json", texture));
            raw(record, 9, 0, id);
            include(spriteIds, texture.getFileIds());
        }
        animationClosure();
        for (int id : fontIds) font(id);
        for (int id : spriteIds) sprite(id);
        for (JsonElement input : request.getAsJsonArray("music"))
        {
            JsonObject music = input.getAsJsonObject();
            music(music.get("id").getAsInt(), music.get("name").getAsString());
        }
        for (JsonElement id : request.getAsJsonArray("sound_ids")) soundIds.add(id.getAsInt());
        for (int id : soundIds) sound(id);
        title();
        musicSynthesisInputs();
        json("bundle.json", map("schema_version", 1, "cache_id", cacheId, "revision", revision,
            "request", request, "counts", counts, "groups", groupSources, "records", records,
            "closure", map("requested_map_squares", regions,
                "map_squares_absent_from_source_index", absentMapSquares,
                "cached_animation_ids_preserved_as_original_bytes", cachedAnimationIds,
                "npc_spawns_and_server_behaviour", "not contained in this cache",
                "dynamic_interfaces_and_clientscript_dependencies", "not yet transitively closed",
                "music_instrument_samples_and_playback", "all index-14 samples/setup and index-15 patches preserved; source synthesis/playback verification pending",
                "owner_reference_pack_approved", false, "source_game_captures", false)));
        System.out.println(GSON.toJson(counts));
    }

    public void close() throws IOException { store.close(); }

    public static void main(String[] args) throws Exception
    {
        ImageIO.setUseCache(false);
        if (args.length < 5 || !(args[0].equals("scan") || args[0].equals("extract")))
        {
            throw new IllegalArgumentException("Usage: CacheExtractor scan|extract CACHE OUTPUT REVISION CACHE_ID [REQUEST]");
        }
        try (CacheExtractor extractor = new CacheExtractor(Path.of(args[1]), Path.of(args[2]),
            Integer.parseInt(args[3]), Integer.parseInt(args[4])))
        {
            if (args[0].equals("scan")) extractor.scan();
            else
            {
                if (args.length != 6) throw new IllegalArgumentException("extract requires a request JSON");
                JsonObject request = new JsonParser().parse(Files.readString(Path.of(args[5]))).getAsJsonObject();
                if (request.get("cache_id").getAsInt() != extractor.cacheId
                    || request.get("game_revision").getAsInt() != extractor.revision)
                {
                    throw new IllegalArgumentException("Request disagrees with selected cache identity");
                }
                extractor.extract(request);
            }
        }
    }
}
