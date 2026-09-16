use clubscape_game_types::{InstanceId, InstanceTemplateId, RegionId};
use clubscape_protocol::game;
use serde_json::{Value, json};

use crate::BridgeError;

pub(crate) fn project(
    scene: &game::CurrentScene,
    player: &game::Player,
) -> Result<Value, BridgeError> {
    let invalid = || {
        BridgeError::protocol(
            "The actual scene region/opaque instance/template pair is inconsistent.",
        )
    };
    RegionId::new(&scene.region).map_err(|_| invalid())?;
    if scene.region != player.region
        || scene.instance != player.instance
        || scene.instance.is_some() != scene.instance_template.is_some()
    {
        return Err(invalid());
    }
    if let Some(instance) = &scene.instance {
        InstanceId::new(instance).map_err(|_| invalid())?;
    }
    if let Some(template) = &scene.instance_template {
        InstanceTemplateId::new(template).map_err(|_| invalid())?;
    }
    Ok(
        json!({"region":scene.region,"instance":scene.instance,"instanceTemplate":scene.instance_template}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_forwards_the_actual_template_without_interpreting_opaque_instance_text() {
        let player = game::Player {
            region: "region.osrs.12633".into(),
            instance: Some("instance.opaque_42".into()),
            ..Default::default()
        };
        let mut scene = game::CurrentScene {
            region: player.region.clone(),
            instance: player.instance.clone(),
            instance_template: Some("instance_template.death.office".into()),
        };
        let view = project(&scene, &player).unwrap();
        assert_eq!(view["instance"], "instance.opaque_42");
        assert_eq!(view["instanceTemplate"], "instance_template.death.office");
        scene.instance_template = None;
        assert!(project(&scene, &player).is_err());
        scene.instance_template = Some("instance_template.death.office".into());
        scene.region = "region.other".into();
        assert!(project(&scene, &player).is_err());
    }
}
