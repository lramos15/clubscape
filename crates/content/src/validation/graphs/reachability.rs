use super::*;

#[derive(Clone, Default)]
struct EventContext {
    tutorial: Option<StageId>,
    quest: Option<(QuestId, StageId)>,
}

impl EventContext {
    fn borrowed(&self) -> Context<'_> {
        Context {
            tutorial: self.tutorial.as_ref(),
            quest: self.quest.as_ref().map(|(quest, stage)| (quest, stage)),
        }
    }
}

struct Reachable {
    tutorial: BTreeSet<StageId>,
    quests: BTreeMap<QuestId, BTreeSet<StageId>>,
    flags: BTreeMap<String, BTreeSet<i64>>,
    interfaces: BTreeSet<InterfaceId>,
    tutorial_events: BTreeSet<StageId>,
    quest_events: BTreeSet<(QuestId, StageId)>,
}

pub(super) fn analyze(
    validator: &Validator<'_>,
    sites: &[Site<'_>],
    budget: &mut Budget,
) -> GameResult<BTreeSet<usize>> {
    let initial = &validator.content.initial_state;
    let mut reachable = Reachable {
        tutorial: BTreeSet::from([initial.tutorial_stage.clone()]),
        quests: initial
            .quests
            .iter()
            .map(|(id, state)| (id.clone(), BTreeSet::from([state.stage.clone()])))
            .collect(),
        flags: initial
            .flags
            .iter()
            .map(|(name, value)| (name.clone(), BTreeSet::from([*value])))
            .collect(),
        interfaces: initial.interfaces.iter().cloned().collect(),
        tutorial_events: BTreeSet::new(),
        quest_events: BTreeSet::new(),
    };
    let mut enabled = BTreeSet::new();
    loop {
        let mut changed = false;
        for (index, site) in sites.iter().enumerate() {
            budget.step()?;
            if site
                .tutorial_owner
                .is_some_and(|owner| !reachable.tutorial.contains(owner))
            {
                continue;
            }
            for mut context in reachable.contexts(site, budget)? {
                if let Some(owner) = site.tutorial_owner {
                    if context
                        .tutorial
                        .as_ref()
                        .is_some_and(|current| current != owner)
                    {
                        continue;
                    }
                    context.tutorial = Some(owner.clone());
                }
                if reachable.possible(validator, &site.guards, context.borrowed(), budget)? {
                    changed |= enabled.insert(index);
                    changed |= reachable.effects(
                        validator,
                        site.effects,
                        context,
                        &site.guards,
                        budget,
                    )?;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for stage in validator.content.tutorial.keys() {
        if !reachable.tutorial.contains(stage) {
            return Err(invalid(
                "tutorial",
                format!(
                    "required stage {stage} is unreachable from initial state and rooted event/flag/interface dependencies"
                ),
            ));
        }
    }
    for (id, quest) in &validator.content.quests {
        for stage in quest.journal.keys() {
            if !reachable.quests[id].contains(stage) {
                return Err(invalid(
                    &format!("quests.{id}"),
                    format!(
                        "required stage {stage} is unreachable from initial state and rooted event/flag/interface dependencies"
                    ),
                ));
            }
        }
    }
    Ok(enabled)
}

impl Reachable {
    fn contexts(&self, site: &Site<'_>, budget: &mut Budget) -> GameResult<Vec<EventContext>> {
        let Some(transition) = site.transition else {
            return Ok(vec![EventContext::default()]);
        };
        let mut contexts = Vec::new();
        match transition.event.as_str() {
            "tutorial_advanced" => {
                for stage in &self.tutorial_events {
                    budget.step()?;
                    if transition
                        .target
                        .as_deref()
                        .is_none_or(|target| target == stage.as_str())
                    {
                        contexts.push(EventContext {
                            tutorial: Some(stage.clone()),
                            quest: None,
                        });
                    }
                }
            }
            "quest_advanced" => {
                for (quest, stage) in &self.quest_events {
                    budget.step()?;
                    if transition
                        .target
                        .as_deref()
                        .is_none_or(|target| target == quest.as_str())
                    {
                        contexts.push(EventContext {
                            tutorial: None,
                            quest: Some((quest.clone(), stage.clone())),
                        });
                    }
                }
            }
            _ => contexts.push(EventContext::default()),
        }
        Ok(contexts)
    }

    fn effects(
        &mut self,
        validator: &Validator<'_>,
        effects: &[Effect],
        mut context: EventContext,
        guards: &[&Guard],
        budget: &mut Budget,
    ) -> GameResult<bool> {
        let mut changed = false;
        let mut active = guards.to_vec();
        for effect in effects {
            budget.step()?;
            match effect {
                Effect::SetTutorialStage { stage } => {
                    changed |= self.tutorial.insert(stage.clone());
                    changed |= self.tutorial_events.insert(stage.clone());
                    context.tutorial = Some(stage.clone());
                    active.clear();
                }
                Effect::SetQuestStage { quest, stage } => {
                    changed |= self
                        .quests
                        .get_mut(quest)
                        .expect("validated quest")
                        .insert(stage.clone());
                    changed |= self.quest_events.insert((quest.clone(), stage.clone()));
                    context.quest = Some((quest.clone(), stage.clone()));
                    active.clear();
                }
                Effect::SetFlag { name, value } => {
                    changed |= self
                        .flags
                        .get_mut(name)
                        .expect("declared flag")
                        .insert(*value);
                    active.clear();
                }
                Effect::UnlockInterface { interface } => {
                    changed |= self.interfaces.insert(interface.clone());
                    active.clear();
                }
                Effect::Conditional { guard, effects } => {
                    let mut guards = active.clone();
                    guards.push(guard);
                    if self.possible(validator, &guards, context.borrowed(), budget)? {
                        changed |=
                            self.effects(validator, effects, context.clone(), &guards, budget)?;
                        // A conditional write is not guaranteed. Widen rather than
                        // wrongly assuming either its old or new state afterward.
                        let mut tutorial = false;
                        let mut quests = BTreeSet::new();
                        collect_changed(effects, &mut tutorial, &mut quests);
                        if tutorial {
                            context.tutorial = None;
                        }
                        if context
                            .quest
                            .as_ref()
                            .is_some_and(|(quest, _)| quests.contains(quest))
                        {
                            context.quest = None;
                        }
                        if mutates_guards(effects) {
                            active.clear();
                        }
                    }
                }
                Effect::Message { .. } | Effect::AddQuestPoints { .. } => {}
                _ => active.clear(),
            }
        }
        Ok(changed)
    }

    fn possible(
        &self,
        validator: &Validator<'_>,
        guards: &[&Guard],
        context: Context<'_>,
        budget: &mut Budget,
    ) -> GameResult<bool> {
        if !consistent(guards, budget)? {
            return Ok(false);
        }
        for guard in guards {
            if self.truth(validator, guard, context, budget)? == Truth::False {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn truth(
        &self,
        validator: &Validator<'_>,
        guard: &Guard,
        context: Context<'_>,
        budget: &mut Budget,
    ) -> GameResult<Truth> {
        budget.step()?;
        Ok(match guard {
            Guard::Always => Truth::True,
            Guard::All { guards } => {
                if !consistent(&guards.iter().collect::<Vec<_>>(), budget)? {
                    return Ok(Truth::False);
                }
                let mut result = Truth::True;
                for guard in guards {
                    match self.truth(validator, guard, context, budget)? {
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
                    match self.truth(validator, guard, context, budget)? {
                        Truth::True => return Ok(Truth::True),
                        Truth::Unknown => result = Truth::Unknown,
                        Truth::False => {}
                    }
                }
                result
            }
            Guard::Not { guard } => self.truth(validator, guard, context, budget)?.not(),
            Guard::TutorialStage { stage } => match context.tutorial {
                Some(current) => known(Some(current == stage)),
                None => domain(&self.tutorial, stage),
            },
            Guard::QuestStage { quest, stage } => match context.quest {
                Some((id, current)) if id == quest => known(Some(current == stage)),
                _ => domain(&self.quests[quest], stage),
            },
            Guard::Flag { name, equals } => domain(&self.flags[name], equals),
            Guard::InterfaceUnlocked { interface } => {
                if validator
                    .content
                    .initial_state
                    .interfaces
                    .contains(interface)
                {
                    Truth::True
                } else if self.interfaces.contains(interface) {
                    Truth::Unknown
                } else {
                    Truth::False
                }
            }
            _ => Truth::Unknown,
        })
    }
}

fn domain<T: Ord>(values: &BTreeSet<T>, required: &T) -> Truth {
    if !values.contains(required) {
        Truth::False
    } else if values.len() == 1 {
        Truth::True
    } else {
        Truth::Unknown
    }
}
