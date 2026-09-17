use super::*;

impl Validator<'_> {
    pub(super) fn audio_authority(&self, audio: &AudioAuthorityDefinition) -> GameResult<()> {
        text(&audio.profile, "ui.audio_authority.profile", 160)?;
        if audio.version != AUDIO_AUTHORITY_VERSION
            || audio.tracks.is_empty()
            || audio.tracks.len() > 128
            || audio.areas.len() > 32
            || audio.varps.len() > 64
        {
            return Err(invalid(
                "ui.audio_authority",
                "invalid version or bounded source registries",
            ));
        }
        for (id, area) in &audio.areas {
            token(id, "ui.audio_authority.area")?;
            if area.plane > 3 || area.polygons.is_empty() || area.polygons.len() > 16 {
                return Err(invalid(
                    "ui.audio_authority.area",
                    "invalid source plane or polygons",
                ));
            }
            for polygon in &area.polygons {
                if polygon.len() < 3
                    || polygon.len() > 2048
                    || polygon
                        .iter()
                        .any(|point| point.iter().any(|v| !(0..=32766).contains(v)))
                    || polygon.iter().collect::<BTreeSet<_>>().len() < 3
                {
                    return Err(invalid(
                        "ui.audio_authority.area",
                        "invalid half-tile source polygon",
                    ));
                }
            }
        }
        for (group, track) in &audio.tracks {
            text(&track.name, "ui.audio_authority.track.name", 128)?;
            if *group == 0
                || *group > 65534
                || track.group != *group
                || track
                    .area
                    .as_ref()
                    .is_some_and(|id| !audio.areas.contains_key(id))
                || (!track.automatic && track.area.is_none() && track.conserved.is_none())
            {
                return Err(invalid(
                    "ui.audio_authority.track",
                    "title music, unknown area or missing unlock evidence",
                ));
            }
            if let Some(id) = &track.conserved {
                let counter = self.content.mechanics.counters.get(id).ok_or_else(|| {
                    invalid("ui.audio_authority.track", "unknown conserved source fact")
                })?;
                if counter.scope != CounterScope::Character
                    || counter.value_type != CounterType::Boolean
                {
                    return Err(invalid(
                        "ui.audio_authority.track",
                        "conserved music facts must be character booleans",
                    ));
                }
            }
        }
        for (id, variable) in &audio.varps {
            token(&variable.binding, "ui.audio_authority.varp.binding")?;
            if *id > 65534 || variable.fields.is_empty() || variable.fields.len() > 32 {
                return Err(invalid(
                    "ui.audio_authority.varp",
                    "invalid bounded native variable",
                ));
            }
            let mut used = 0_u32;
            for field in &variable.fields {
                if field.width == 0
                    || field.width > 32
                    || u16::from(field.lsb) + u16::from(field.width) > 32
                    || field.cases.is_empty()
                    || field.cases.len() > 256
                {
                    return Err(invalid(
                        "ui.audio_authority.varp",
                        "invalid field width or source cases",
                    ));
                }
                let mask = u32::MAX >> (32 - field.width);
                let shifted = mask << field.lsb;
                if used & shifted != 0 {
                    return Err(invalid(
                        "ui.audio_authority.varp",
                        "overlapping source bit fields",
                    ));
                }
                used |= shifted;
                for case in &field.cases {
                    self.guard(&case.guard, "ui.audio_authority.varp.guard")?;
                    if case.value > mask {
                        return Err(invalid(
                            "ui.audio_authority.varp",
                            "source value does not fit its field",
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
