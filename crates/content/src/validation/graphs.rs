use std::collections::VecDeque;

use super::*;

mod reachability;

type Graph = BTreeMap<StageId, BTreeSet<StageId>>;
const MAX_GRAPH_WORK: usize = 5_000_000;

#[derive(Clone, Copy, Default)]
struct Context<'a> {
    tutorial: Option<&'a StageId>,
    quest: Option<(&'a QuestId, &'a StageId)>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Truth {
    True,
    False,
    Unknown,
}

impl Truth {
    fn not(self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }
}

struct Budget(usize);

impl Budget {
    fn step(&mut self) -> GameResult<()> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or_else(|| invalid("graphs", "static graph-analysis work budget exceeded"))?;
        Ok(())
    }
}

struct Site<'a> {
    path: String,
    tutorial_owner: Option<&'a StageId>,
    quest_owner: Option<&'a QuestId>,
    guards: Vec<&'a Guard>,
    effects: &'a [Effect],
    transition: Option<&'a ProgressTransition>,
}

impl Validator<'_> {
    pub(super) fn dialogues(&self) -> GameResult<BTreeMap<DialogueId, BTreeMap<String, usize>>> {
        let mut indexes = BTreeMap::new();
        let mut budget = Budget(MAX_GRAPH_WORK);
        for dialogue in self.content.dialogues.values() {
            let path = format!("dialogues.{}", dialogue.id);
            nonempty(dialogue.nodes.len(), &format!("{path}.nodes"))?;
            nonempty(dialogue.entry_nodes.len(), &format!("{path}.entry_nodes"))?;
            unique(dialogue.entry_nodes.iter(), &path)?;
            let mut nodes = BTreeMap::new();
            for (index, node) in dialogue.nodes.iter().enumerate() {
                token(&node.id, &path)?;
                text(
                    &node.text,
                    &format!("{path}.{}.text", node.id),
                    MAX_TEXT_BYTES,
                )?;
                if nodes.insert(node.id.clone(), index).is_some() {
                    return Err(invalid(
                        &path,
                        format!("duplicate dialogue node {}", node.id),
                    ));
                }
                self.state_guard(&node.guard, &format!("{path}.{}.guard", node.id))?;
                unique(node.choices.iter().map(|choice| &choice.id), &path)?;
                for choice in &node.choices {
                    let path = format!("{path}.{}.choices.{}", node.id, choice.id);
                    token(&choice.id, &path)?;
                    text(&choice.text, &path, MAX_TEXT_BYTES)?;
                    self.state_guard(&choice.guard, &path)?;
                    self.state_effects(&choice.effects, &path)?;
                }
            }
            let mut reachable = BTreeSet::new();
            let mut queue = VecDeque::new();
            for entry in &dialogue.entry_nodes {
                let index = nodes
                    .get(entry)
                    .ok_or_else(|| invalid(&path, format!("undefined entry node {entry}")))?;
                if self.possible(
                    &[&dialogue.nodes[*index].guard],
                    Context::default(),
                    &mut budget,
                )? {
                    reachable.insert(entry.clone());
                    queue.push_back(entry.clone());
                }
            }
            for node in &dialogue.nodes {
                for choice in &node.choices {
                    if let Some(next) = &choice.next_node
                        && !nodes.contains_key(next)
                    {
                        return Err(invalid(
                            &path,
                            format!(
                                "node {} choice {} has undefined next node {next}",
                                node.id, choice.id
                            ),
                        ));
                    }
                }
            }
            while let Some(id) = queue.pop_front() {
                let node = &dialogue.nodes[nodes[&id]];
                for choice in &node.choices {
                    if let Some(next) = &choice.next_node {
                        let next_node = &dialogue.nodes[nodes[next]];
                        if self.possible(
                            &[&node.guard, &choice.guard],
                            Context::default(),
                            &mut budget,
                        )? && self.possible(
                            &[&next_node.guard],
                            Context::default(),
                            &mut budget,
                        )? && reachable.insert(next.clone())
                        {
                            queue.push_back(next.clone());
                        }
                    }
                }
            }
            if let Some(unreachable) = nodes.keys().find(|node| !reachable.contains(*node)) {
                return Err(invalid(
                    &path,
                    format!("required dialogue node {unreachable} is unreachable from the entries"),
                ));
            }
            indexes.insert(dialogue.id.clone(), nodes);
        }
        Ok(indexes)
    }

    pub(super) fn progression_definitions(&self) -> GameResult<()> {
        if self.content.tutorial.len() < 2 {
            return Err(invalid(
                "tutorial",
                "an initial tutorial needs progression, not a single pre-completed stage",
            ));
        }
        for stage in self.content.tutorial.values() {
            let path = format!("tutorial.{}", stage.id);
            text(&stage.instruction, &path, MAX_TEXT_BYTES)?;
            unique(stage.allowed_actions.iter(), &path)?;
            for action in &stage.allowed_actions {
                self.allowed_action(action, &path)?;
            }
            bounded(stage.xp_caps_tenths.len(), &path)?;
            for (id, cap) in &stage.xp_caps_tenths {
                if *cap > self.skill(id, &path)?.maximum_xp_tenths {
                    return Err(invalid(
                        &path,
                        format!("XP cap exceeds maximum XP for {id}"),
                    ));
                }
            }
            bounded(stage.xp_stop_levels.len(), &path)?;
            for (skill, level) in &stage.xp_stop_levels {
                self.requirement(
                    &SkillRequirement {
                        skill: skill.clone(),
                        level: *level,
                        basis: SkillLevelBasis::Base,
                    },
                    &path,
                )?;
            }
            for (index, transition) in stage.transitions.iter().enumerate() {
                self.transition(transition, &format!("{path}.transitions[{index}]"))?;
            }
        }
        for quest in self.content.quests.values() {
            let path = format!("quests.{}", quest.id);
            text(&quest.name, &path, 256)?;
            nonempty(quest.journal.len(), &format!("{path}.journal"))?;
            self.quest_stage(&quest.id, &quest.initial_stage, &path)?;
            self.quest_stage(&quest.id, &quest.completed_stage, &path)?;
            if quest.initial_stage == quest.completed_stage {
                return Err(invalid(
                    &path,
                    "initial and completed quest stages must be distinct",
                ));
            }
            for entry in quest.journal.values() {
                text(entry, &path, MAX_TEXT_BYTES)?;
            }
            for (index, transition) in quest.transitions.iter().enumerate() {
                self.transition(transition, &format!("{path}.transitions[{index}]"))?;
            }
        }
        Ok(())
    }

    pub(super) fn progression_graphs(&self) -> GameResult<()> {
        let sites = self.sites();
        let mut tutorial: Graph = self
            .content
            .tutorial
            .keys()
            .map(|id| (id.clone(), BTreeSet::new()))
            .collect();
        let mut quests: BTreeMap<_, Graph> = self
            .content
            .quests
            .iter()
            .map(|(id, quest)| {
                (
                    id.clone(),
                    quest
                        .journal
                        .keys()
                        .map(|stage| (stage.clone(), BTreeSet::new()))
                        .collect(),
                )
            })
            .collect();
        let mut budget = Budget(MAX_GRAPH_WORK);
        for site in &sites {
            single_stage_writes(site, &mut budget)?;
            let mut changed_quests = BTreeSet::new();
            let mut changes_tutorial = false;
            collect_changed(site.effects, &mut changes_tutorial, &mut changed_quests);
            self.reward_branches(
                site,
                site.effects,
                &site.guards,
                &BTreeMap::new(),
                &changed_quests,
                &mut budget,
            )?;
        }
        let enabled = reachability::analyze(self, &sites, &mut budget)?;
        for (index, site) in sites.iter().enumerate() {
            if !enabled.contains(&index) {
                continue;
            }
            let mut changed_quests = BTreeSet::new();
            let mut changes_tutorial = false;
            collect_changed(site.effects, &mut changes_tutorial, &mut changed_quests);
            if changes_tutorial {
                for stage in self.content.tutorial.keys() {
                    budget.step()?;
                    if site.tutorial_owner.is_some_and(|owner| owner != stage) {
                        continue;
                    }
                    let context = Context {
                        tutorial: Some(stage),
                        quest: None,
                    };
                    if self.possible(&site.guards, context, &mut budget)? {
                        let mut targets = BTreeSet::new();
                        self.destinations(
                            site.effects,
                            context,
                            None,
                            &site.guards,
                            &mut targets,
                            &mut budget,
                        )?;
                        targets.remove(stage);
                        tutorial
                            .get_mut(stage)
                            .expect("defined tutorial stage")
                            .extend(targets);
                    }
                }
            }
            for quest_id in changed_quests {
                let quest = &self.content.quests[quest_id];
                for stage in quest.journal.keys() {
                    budget.step()?;
                    let context = Context {
                        tutorial: site.tutorial_owner,
                        quest: Some((quest_id, stage)),
                    };
                    if self.possible(&site.guards, context, &mut budget)? {
                        let mut targets = BTreeSet::new();
                        self.destinations(
                            site.effects,
                            context,
                            Some(quest_id),
                            &site.guards,
                            &mut targets,
                            &mut budget,
                        )?;
                        targets.remove(stage);
                        if stage == &quest.completed_stage && !targets.is_empty() {
                            return Err(invalid(
                                &site.path,
                                format!(
                                    "completed quest {quest_id} can be reset or advanced again"
                                ),
                            ));
                        }
                        quests
                            .get_mut(quest_id)
                            .expect("defined quest")
                            .get_mut(stage)
                            .expect("defined quest stage")
                            .extend(targets);
                    }
                }
            }
        }
        check_reachable(
            &tutorial,
            &self.content.initial_state.tutorial_stage,
            "tutorial",
        )?;
        let terminal: BTreeSet<_> = tutorial
            .iter()
            .filter(|(_, edges)| edges.is_empty())
            .map(|(id, _)| id.clone())
            .collect();
        check_can_finish(&tutorial, &terminal, "tutorial")?;
        for (id, graph) in quests {
            let quest = &self.content.quests[&id];
            let path = format!("quests.{id}");
            check_reachable(&graph, &quest.initial_stage, &path)?;
            check_can_finish(
                &graph,
                &BTreeSet::from([quest.completed_stage.clone()]),
                &path,
            )?;
        }
        Ok(())
    }

    fn sites(&self) -> Vec<Site<'_>> {
        let mut sites = Vec::new();
        for object in self.content.mechanics.temporary_objects.values() {
            for interaction in &object.interactions {
                if let InteractionAction::Effects { effects } = &interaction.action {
                    sites.push(Site {
                        path: format!(
                            "mechanics.temporary_objects.{}.{}",
                            object.id, interaction.name
                        ),
                        tutorial_owner: None,
                        quest_owner: None,
                        guards: vec![&object.placement_guard, &interaction.guard],
                        effects,
                        transition: None,
                    });
                }
            }
        }
        for spawn in self.content.spawns.values() {
            for interaction in &spawn.interactions {
                if let InteractionAction::Effects { effects }
                | InteractionAction::OpenBank {
                    before_open: effects,
                    ..
                }
                | InteractionAction::OpenShop {
                    before_open: effects,
                    ..
                } = &interaction.action
                {
                    sites.push(Site {
                        path: format!("spawns.{}.{}", spawn.id, interaction.name),
                        tutorial_owner: None,
                        quest_owner: None,
                        guards: vec![&interaction.guard],
                        effects,
                        transition: None,
                    });
                }
                for recipe in self.content.recipes.values() {
                    if let Some(mechanics) = &recipe.mechanics {
                        for (outcome, effects) in [
                            ("success", &mechanics.success_effects),
                            ("failure", &mechanics.failure_effects),
                        ] {
                            if !effects.is_empty() {
                                let mut guards = vec![&mechanics.guard];
                                if let Some(SourceBinding::Bound { value, .. }) =
                                    &recipe.item_on_target
                                {
                                    guards.push(&value.guard);
                                }
                                sites.push(Site {
                                    path: format!("recipes.{}.{}", recipe.id, outcome),
                                    tutorial_owner: None,
                                    quest_owner: None,
                                    guards,
                                    effects,
                                    transition: None,
                                });
                            }
                        }
                    }
                }
                for travel in self.content.mechanics.travels.values() {
                    if !travel.completion_effects.is_empty() {
                        sites.push(Site {
                            path: format!("mechanics.travels.{}", travel.id),
                            tutorial_owner: None,
                            quest_owner: None,
                            guards: vec![&travel.guard],
                            effects: &travel.completion_effects,
                            transition: None,
                        });
                    }
                }
            }
        }
        for dialogue in self.content.dialogues.values() {
            for node in &dialogue.nodes {
                for choice in &node.choices {
                    if !choice.effects.is_empty() {
                        sites.push(Site {
                            path: format!("dialogues.{}.{}.{}", dialogue.id, node.id, choice.id),
                            tutorial_owner: None,
                            quest_owner: None,
                            guards: vec![&node.guard, &choice.guard],
                            effects: &choice.effects,
                            transition: None,
                        });
                    }
                }
            }
        }
        for stage in self.content.tutorial.values() {
            for (index, transition) in stage.transitions.iter().enumerate() {
                sites.push(Site {
                    path: format!("tutorial.{}.transitions[{index}]", stage.id),
                    tutorial_owner: Some(&stage.id),
                    quest_owner: None,
                    guards: vec![&transition.guard],
                    effects: &transition.effects,
                    transition: Some(transition),
                });
            }
        }
        for quest in self.content.quests.values() {
            for (index, transition) in quest.transitions.iter().enumerate() {
                sites.push(Site {
                    path: format!("quests.{}.transitions[{index}]", quest.id),
                    tutorial_owner: None,
                    quest_owner: Some(&quest.id),
                    guards: vec![&transition.guard],
                    effects: &transition.effects,
                    transition: Some(transition),
                });
            }
        }
        sites
    }

    fn destinations<'a>(
        &self,
        effects: &'a [Effect],
        mut context: Context<'a>,
        quest: Option<&QuestId>,
        guards: &[&'a Guard],
        targets: &mut BTreeSet<StageId>,
        budget: &mut Budget,
    ) -> GameResult<()> {
        let mut active = guards.to_vec();
        for effect in effects {
            budget.step()?;
            match effect {
                Effect::SetTutorialStage { stage } => {
                    if quest.is_none() {
                        targets.insert(stage.clone());
                    }
                    context.tutorial = Some(stage);
                    active.clear();
                }
                Effect::SetQuestStage { quest: id, stage } => {
                    if quest == Some(id) {
                        targets.insert(stage.clone());
                    }
                    if context.quest.is_none_or(|(current, _)| current == id) {
                        context.quest = Some((id, stage));
                    }
                    active.clear();
                }
                Effect::Conditional { guard, effects } => {
                    let mut guards = active.clone();
                    guards.push(guard);
                    if self.possible(&guards, context, budget)? {
                        self.destinations(effects, context, quest, &guards, targets, budget)?;
                        let mut tutorial = false;
                        let mut quests = BTreeSet::new();
                        collect_changed(effects, &mut tutorial, &mut quests);
                        if tutorial {
                            context.tutorial = None;
                        }
                        if context.quest.is_some_and(|(id, _)| quests.contains(id)) {
                            context.quest = None;
                        }
                        if mutates_guards(effects) {
                            active.clear();
                        }
                    }
                }
                Effect::Once { effects, .. } => {
                    self.destinations(effects, context, quest, &active, targets, budget)?;
                    active.clear();
                }
                Effect::Message { .. } | Effect::AddQuestPoints { .. } => {}
                _ => active.clear(),
            }
        }
        Ok(())
    }

    fn reward_branches<'a>(
        &self,
        site: &Site<'a>,
        effects: &'a [Effect],
        guards: &[&Guard],
        inherited: &BTreeMap<&'a QuestId, &'a StageId>,
        changed: &BTreeSet<&QuestId>,
        budget: &mut Budget,
    ) -> GameResult<()> {
        budget.step()?;
        let mut destinations = inherited.clone();
        for effect in effects {
            if let Effect::SetQuestStage { quest, stage } = effect {
                destinations.insert(quest, stage);
            }
        }
        let has_reward = effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::GiveItems { .. }
                    | Effect::AwardXp { .. }
                    | Effect::AddQuestPoints { .. }
                    | Effect::Grant { .. }
                    | Effect::RestoreVital { .. }
                    | Effect::ReconcileContainers { .. },
            )
        });
        if has_reward {
            if let Some(owner) = site.quest_owner
                && !destinations.contains_key(owner)
            {
                return Err(invalid(
                    &site.path,
                    "quest reward needs an unconditional destination in the same effect branch",
                ));
            }
            if (!changed.is_empty()
                || effects
                    .iter()
                    .any(|effect| matches!(effect, Effect::AddQuestPoints { .. })))
                && destinations.is_empty()
            {
                return Err(invalid(
                    &site.path,
                    "reward has no guaranteed quest progression destination",
                ));
            }
            for (quest, destination) in &destinations {
                let context = Context {
                    tutorial: site.tutorial_owner,
                    quest: Some((quest, destination)),
                };
                if self.possible(guards, context, budget)? {
                    return Err(invalid(
                        &site.path,
                        format!(
                            "quest reward for {quest} is repeatable: its guard remains possible at destination {destination}",
                        ),
                    ));
                }
            }
        }
        for effect in effects {
            if let Effect::Conditional { guard, effects } = effect {
                let mut guards = guards.to_vec();
                guards.push(guard);
                self.reward_branches(site, effects, &guards, &destinations, changed, budget)?;
            } else if let Effect::Once { effects, .. } = effect {
                self.reward_branches(site, effects, guards, &destinations, changed, budget)?;
            }
        }
        Ok(())
    }

    fn possible(
        &self,
        guards: &[&Guard],
        context: Context<'_>,
        budget: &mut Budget,
    ) -> GameResult<bool> {
        if !consistent(guards, budget)? {
            return Ok(false);
        }
        for guard in guards {
            if self.truth(guard, context, budget)? == Truth::False {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn truth(&self, guard: &Guard, context: Context<'_>, budget: &mut Budget) -> GameResult<Truth> {
        budget.step()?;
        let truth = match guard {
            Guard::Always => Truth::True,
            Guard::All { guards } => {
                if !consistent(&guards.iter().collect::<Vec<_>>(), budget)? {
                    return Ok(Truth::False);
                }
                let mut result = Truth::True;
                for guard in guards {
                    match self.truth(guard, context, budget)? {
                        Truth::False => return Ok(Truth::False),
                        Truth::Unknown => result = Truth::Unknown,
                        Truth::True => {}
                    }
                }
                result
            }
            Guard::Any { guards } => {
                let mut result = Truth::False;
                for guard in guards {
                    match self.truth(guard, context, budget)? {
                        Truth::True => return Ok(Truth::True),
                        Truth::Unknown => result = Truth::Unknown,
                        Truth::False => {}
                    }
                }
                result
            }
            Guard::Not { guard } => self.truth(guard, context, budget)?.not(),
            Guard::TutorialStage { stage } => {
                known(context.tutorial.map(|current| current == stage))
            }
            Guard::QuestStage { quest, stage } => known(
                context
                    .quest
                    .and_then(|(id, current)| (id == quest).then_some(current == stage)),
            ),
            Guard::Flag { name, equals } if !self.mutable_flags.contains(name) => known(
                self.content
                    .initial_state
                    .flags
                    .get(name)
                    .map(|current| current == equals),
            ),
            Guard::InterfaceUnlocked { interface }
                if self.content.initial_state.interfaces.contains(interface) =>
            {
                Truth::True
            }
            _ => Truth::Unknown,
        };
        Ok(truth)
    }
}

fn collect_changed<'a>(
    effects: &'a [Effect],
    tutorial: &mut bool,
    quests: &mut BTreeSet<&'a QuestId>,
) {
    for effect in effects {
        match effect {
            Effect::SetTutorialStage { .. } => *tutorial = true,
            Effect::SetQuestStage { quest, .. } => {
                quests.insert(quest);
            }
            Effect::Conditional { effects, .. } | Effect::Once { effects, .. } => {
                collect_changed(effects, tutorial, quests)
            }
            _ => {}
        }
    }
}

fn mutates_guards(effects: &[Effect]) -> bool {
    effects.iter().any(|effect| match effect {
        Effect::Conditional { effects, .. } | Effect::Once { effects, .. } => {
            mutates_guards(effects)
        }
        Effect::Message { .. } | Effect::AddQuestPoints { .. } => false,
        _ => true,
    })
}

fn known(value: Option<bool>) -> Truth {
    match value {
        Some(true) => Truth::True,
        Some(false) => Truth::False,
        None => Truth::Unknown,
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Key<'a> {
    Tutorial,
    Quest(&'a QuestId),
    Flag(&'a str),
    Interface(&'a InterfaceId),
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Value<'a> {
    Stage(&'a StageId),
    Flag(i64),
    Unlocked,
}

fn consistent(guards: &[&Guard], budget: &mut Budget) -> GameResult<bool> {
    let mut atoms = Vec::new();
    for guard in guards {
        atoms_for(guard, false, &mut atoms, budget)?;
    }
    let mut equalities = BTreeMap::new();
    let mut exclusions = BTreeSet::new();
    for (key, value, negated) in atoms {
        if negated {
            exclusions.insert((key, value));
        } else if equalities.get(&key).is_some_and(|old| old != &value) {
            return Ok(false);
        } else {
            equalities.insert(key, value);
        }
    }
    Ok(!equalities
        .into_iter()
        .any(|pair| exclusions.contains(&pair)))
}

fn atoms_for<'a>(
    guard: &'a Guard,
    negated: bool,
    atoms: &mut Vec<(Key<'a>, Value<'a>, bool)>,
    budget: &mut Budget,
) -> GameResult<()> {
    budget.step()?;
    match guard {
        Guard::All { guards } if !negated => {
            for guard in guards {
                atoms_for(guard, false, atoms, budget)?;
            }
        }
        Guard::Not { guard } => atoms_for(guard, !negated, atoms, budget)?,
        Guard::TutorialStage { stage } => atoms.push((Key::Tutorial, Value::Stage(stage), negated)),
        Guard::QuestStage { quest, stage } => {
            atoms.push((Key::Quest(quest), Value::Stage(stage), negated))
        }
        Guard::Flag { name, equals } => {
            atoms.push((Key::Flag(name), Value::Flag(*equals), negated))
        }
        Guard::InterfaceUnlocked { interface } => {
            atoms.push((Key::Interface(interface), Value::Unlocked, negated))
        }
        _ => {}
    }
    Ok(())
}

fn single_stage_writes(site: &Site<'_>, budget: &mut Budget) -> GameResult<()> {
    let mut written = BTreeSet::new();
    let mut pending = vec![site.effects];
    while let Some(effects) = pending.pop() {
        for effect in effects {
            budget.step()?;
            let key = match effect {
                Effect::SetTutorialStage { .. } => Some(None),
                Effect::SetQuestStage { quest, .. } => Some(Some(quest)),
                Effect::Conditional { effects, .. } | Effect::Once { effects, .. } => {
                    pending.push(effects);
                    None
                }
                _ => None,
            };
            if let Some(key) = key
                && !written.insert(key)
            {
                return Err(invalid(
                    &site.path,
                    "multiple unconditional or conditional stage writes for one progression owner; split them into separately guarded choices/transitions",
                ));
            }
        }
    }
    Ok(())
}

fn reachable(graph: &Graph, roots: &BTreeSet<StageId>) -> BTreeSet<StageId> {
    let mut seen = roots.clone();
    let mut queue: VecDeque<_> = roots.iter().cloned().collect();
    while let Some(stage) = queue.pop_front() {
        if let Some(edges) = graph.get(&stage) {
            for target in edges {
                if seen.insert(target.clone()) {
                    queue.push_back(target.clone());
                }
            }
        }
    }
    seen
}

fn check_reachable(graph: &Graph, initial: &StageId, path: &str) -> GameResult<()> {
    let seen = reachable(graph, &BTreeSet::from([initial.clone()]));
    if let Some(stage) = graph.keys().find(|stage| !seen.contains(*stage)) {
        return Err(invalid(
            path,
            format!("required progression stage {stage} is unreachable from {initial}"),
        ));
    }
    Ok(())
}

fn check_can_finish(graph: &Graph, terminal: &BTreeSet<StageId>, path: &str) -> GameResult<()> {
    let mut reverse: Graph = graph
        .keys()
        .map(|id| (id.clone(), BTreeSet::new()))
        .collect();
    for (source, targets) in graph {
        for target in targets {
            reverse
                .get_mut(target)
                .expect("validated stage reference")
                .insert(source.clone());
        }
    }
    let seen = reachable(&reverse, terminal);
    if let Some(stage) = graph.keys().find(|stage| !seen.contains(*stage)) {
        return Err(invalid(
            path,
            format!("stage {stage} has no possible path to completion; closed progression loop"),
        ));
    }
    Ok(())
}
