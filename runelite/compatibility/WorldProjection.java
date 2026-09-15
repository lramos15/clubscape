import clubscape.game.v1.Game;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

/** Projection only: never advances the clock, resolves an action, or awards experience. */
final class WorldProjection
{
    private Game.WorldSnapshot snapshot;
    private final Map<String, Game.Entity> entities = new LinkedHashMap<>();
    private final Set<String> eventIds = new LinkedHashSet<>();
    private final Map<String, Long> observedXp = new LinkedHashMap<>();
    private final Map<String, Long> gainedXp = new LinkedHashMap<>();
    private boolean baseline;

    synchronized List<Game.Event> accept(Game.WorldSnapshot next)
    {
        if (!next.hasPlayer() || !next.getPlayer().hasTile() || next.getNextSequence() == 0)
            throw new IllegalArgumentException("Incomplete authoritative snapshot");
        if (snapshot != null && (next.getRevision() < snapshot.getRevision()
            || next.getTick() < snapshot.getTick()
            || !next.getPlayer().getActorId().equals(snapshot.getPlayer().getActorId())))
            throw new IllegalArgumentException("Stale or changed-owner authoritative snapshot");
        if (!baseline && !next.getFullSnapshot())
            throw new IllegalArgumentException("An entity delta cannot establish a baseline");
        if (next.getFullSnapshot()) entities.clear();
        next.getRemovedEntitiesList().forEach(entities::remove);
        next.getEntitiesList().forEach(entity -> entities.put(entity.getId(), entity));
        if (entities.size() > 2048) throw new IllegalArgumentException("Entity bound exceeded");
        if (next.getEventHistoryGap())
            throw new IllegalStateException("Event history gap: first-XP proof must be restarted, not invented");
        List<Game.Event> fresh = new ArrayList<>();
        Map<String, Long> newGains = new LinkedHashMap<>();
        for (Game.Event event : next.getEventsList())
        {
            if (event.getEventId().isBlank()) throw new IllegalArgumentException("Missing committed event identity");
            if (!eventIds.add(event.getEventId())) continue;
            if (!event.getActorId().isEmpty()
                && !event.getActorId().equals(next.getPlayer().getActorId()))
                throw new IllegalArgumentException("Another actor's private event");
            fresh.add(event);
            if (event.getKind().equals("xp_gained"))
            {
                if (event.getSkill().isBlank() || event.getXpTenths() <= 0)
                    throw new IllegalArgumentException("Invalid server XP event");
                newGains.merge(event.getSkill(), event.getXpTenths(), Math::addExact);
            }
        }
        for (Game.Skill skill : next.getPlayer().getSkillsList())
        {
            Long previous = observedXp.put(skill.getId(), skill.getXpTenths());
            if (skill.getXpTenths() < 0 || skill.getBaseLevel() <= 0 || skill.getCurrentLevel() <= 0)
                throw new IllegalArgumentException("Invalid typed skill view");
            if (baseline && previous != null
                && skill.getXpTenths() - previous != newGains.getOrDefault(skill.getId(), 0L))
                throw new IllegalStateException("XP snapshot change does not match committed XP events: " + skill.getId());
        }
        newGains.forEach((skill, value) -> gainedXp.merge(skill, value, Math::addExact));
        while (eventIds.size() > 4096) eventIds.remove(eventIds.iterator().next());
        baseline = true;
        snapshot = next;
        return fresh;
    }

    synchronized Game.WorldSnapshot snapshot() { return snapshot; }
    synchronized List<Game.Entity> entities() { return List.copyOf(entities.values()); }
    synchronized Map<String, Long> gainedXp() { return Map.copyOf(gainedXp); }
}
