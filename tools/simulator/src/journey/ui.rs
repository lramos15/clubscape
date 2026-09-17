use anyhow::{Context, Result, bail, ensure};
use clubscape_protocol::game::{self, gameplay_ui_request::Request, world_input::Action};
use serde_json::json;

use super::{Receipt, Runner, evidence};

impl Runner {
    pub(super) fn ui(&self) -> Result<&game::GameplayUiView> {
        let ui = self
            .snapshot
            .ui
            .as_ref()
            .context("Source UI projection is absent")?;
        ensure!(ui.version == 1, "Unsupported source UI state version");
        Ok(ui)
    }

    pub(super) async fn ui_input(
        &mut self,
        request: Request,
        expected_bank_revision: Option<String>,
    ) -> Result<Receipt> {
        self.input_raw(Action::Ui(game::GameplayUiRequest {
            expected_bank_revision,
            request: Some(request),
        }))
        .await
    }

    pub(super) async fn continue_source_presentations(&mut self) -> Result<()> {
        for _ in 0..16 {
            let view = self.ui()?.clone();
            ensure!(
                view.confirmation.is_none(),
                "A source confirmation requires an explicit scenario decision; it is not automatically accepted"
            );
            if let Some(reward) = view.reward {
                self.evidence.append(
                    "source_reward_card",
                    evidence::message_json_with_defaults("clubscape.game.v1.UiReward", &reward)?,
                )?;
                let continuation = reward
                    .continuation
                    .context("Source reward has no typed continuation")?;
                ensure!(
                    continuation.expected_bank_revision.is_none()
                        && matches!(&continuation.request, Some(Request::Dismiss(dismiss)) if dismiss.id == reward.id),
                    "Unexpected source reward continuation; no control is fabricated or auto-approved"
                );
                self.input_raw(Action::Ui(continuation)).await?;
                ensure!(
                    self.ui()?
                        .reward
                        .as_ref()
                        .is_none_or(|next| next.id != reward.id),
                    "Source reward continuation did not consume the displayed presentation"
                );
                continue;
            }
            if let Some(document) = view.document {
                self.evidence.append(
                    "source_document_opened",
                    evidence::message_json_with_defaults(
                        "clubscape.game.v1.UiDocument",
                        &document,
                    )?,
                )?;
                ensure!(
                    document.pages.len() <= 128,
                    "Source document exceeds page bound"
                );
                for page in 0..document.pages.len() as u32 {
                    if self
                        .ui()?
                        .document
                        .as_ref()
                        .is_some_and(|current| current.page == page)
                    {
                        continue;
                    }
                    self.ui_input(
                        Request::DocumentPage(game::UiDocumentPage {
                            id: document.id.clone(),
                            page,
                        }),
                        None,
                    )
                    .await?;
                    let current = self
                        .ui()?
                        .document
                        .as_ref()
                        .context("Source document disappeared during paging")?;
                    ensure!(
                        current.id == document.id && current.page == page,
                        "Source document page was not acknowledged"
                    );
                }
                self.ui_input(
                    Request::Dismiss(game::UiIdentity {
                        id: document.id.clone(),
                    }),
                    None,
                )
                .await?;
                ensure!(
                    self.ui()?
                        .document
                        .as_ref()
                        .is_none_or(|current| current.id != document.id),
                    "Source document dismissal was not acknowledged"
                );
                continue;
            }
            return Ok(());
        }
        bail!("Source presentation continuation budget exhausted")
    }

    pub(super) async fn select_source_production(
        &mut self,
        recipe: &str,
        target: Option<&game::WorldTarget>,
        mode: game::ProductionMode,
    ) -> Result<()> {
        self.continue_source_presentations().await?;
        let menu = self
            .ui()?
            .production
            .as_ref()
            .context("No actual source production menu is open")?
            .clone();
        let selection = production_selection(&menu, recipe, target, mode)?;
        self.evidence.append("source_production_selection", json!({
            "menu": evidence::message_json_with_defaults("clubscape.game.v1.UiProduction", &menu)?,
            "request": evidence::message_json("clubscape.game.v1.UiProductionSelection", &selection)?,
            "targetless_inventory_menu": target.is_none(),
            "items_and_xp_before_selection": evidence::public_snapshot(&self.snapshot)?
        }))?;
        self.ui_input(Request::Production(selection), None).await?;
        Ok(())
    }

    pub(super) fn production_menu_matches(
        &self,
        recipe: &str,
        target: Option<&game::WorldTarget>,
    ) -> Result<bool> {
        Ok(self.ui()?.production.as_ref().is_some_and(|menu| {
            menu.target.as_ref() == target
                && menu.recipes.iter().any(|choice| choice.recipe == recipe)
        }))
    }

    pub(super) fn bank_revision(&self) -> Result<String> {
        let bank = self
            .ui()?
            .bank
            .as_ref()
            .context("No actual authorized UI bank view")?;
        let revision = bank
            .revision
            .parse::<u64>()
            .context("Invalid source bank revision")?;
        ensure!(
            revision.to_string() == bank.revision,
            "Bank revision is not a canonical decimal"
        );
        Ok(bank.revision.clone())
    }
}

fn production_selection(
    menu: &game::UiProduction,
    recipe: &str,
    expected_target: Option<&game::WorldTarget>,
    mode: game::ProductionMode,
) -> Result<game::UiProductionSelection> {
    ensure!(
        !menu.id.is_empty(),
        "Source production menu identity is missing"
    );
    ensure!(
        menu.target.as_ref() == expected_target,
        "Source production menu target changed; no implicit retargeting"
    );
    let mut choices = menu.recipes.iter().filter(|choice| choice.recipe == recipe);
    let choice = choices
        .next()
        .context("Required source recipe is absent from the actual menu")?;
    ensure!(
        choices.next().is_none(),
        "Source recipe selection is ambiguous"
    );
    let permission = match mode {
        game::ProductionMode::Single => choice.single.as_ref(),
        game::ProductionMode::MakeX => choice.make_x.as_ref(),
        _ => bail!("A real explicit production mode is required"),
    }
    .context("Source production permission was not evaluated")?;
    ensure!(
        permission.allowed,
        "Actual source production mode is denied: {:?}",
        permission.denial
    );
    Ok(game::UiProductionSelection {
        menu_id: menu.id.clone(),
        recipe: recipe.to_owned(),
        quantity: 1,
        mode: mode as i32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use prost::Message;

    fn menu(target: Option<game::WorldTarget>) -> game::UiProduction {
        game::UiProduction {
            id: "ui.7".into(),
            interface: "interface.cooking".into(),
            target,
            recipes: vec![game::UiProductionChoice {
                recipe: "recipe.cooking.dough".into(),
                single: Some(game::Permission {
                    allowed: true,
                    denial: None,
                }),
                make_x: Some(game::Permission {
                    allowed: true,
                    denial: None,
                }),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn inventory_only_production_preserves_absent_target_and_explicit_mode() {
        let menu = menu(None);
        let decoded = game::UiProduction::decode(menu.encode_to_vec().as_slice()).unwrap();
        assert!(decoded.target.is_none());
        for mode in [game::ProductionMode::Single, game::ProductionMode::MakeX] {
            let selected =
                production_selection(&decoded, "recipe.cooking.dough", None, mode).unwrap();
            assert_eq!(selected.menu_id, "ui.7");
            assert_eq!(selected.quantity, 1);
            assert_eq!(selected.mode, mode as i32);
        }
    }

    #[test]
    fn production_selection_cannot_fabricate_targets_permissions_or_recipes() {
        let target = game::WorldTarget {
            target: Some(game::world_target::Target::Spawn(
                "spawn.range.tutorial.3075.3081.p0.t10.r3".into(),
            )),
        };
        assert!(
            production_selection(
                &menu(None),
                "recipe.cooking.dough",
                Some(&target),
                game::ProductionMode::Single
            )
            .is_err()
        );
        let mut actual = menu(Some(target.clone()));
        assert!(
            production_selection(
                &actual,
                "recipe.smelting.bronze",
                Some(&target),
                game::ProductionMode::Single
            )
            .is_err()
        );
        actual.recipes[0].single.as_mut().unwrap().allowed = false;
        assert!(
            production_selection(
                &actual,
                "recipe.cooking.dough",
                Some(&target),
                game::ProductionMode::Single
            )
            .is_err()
        );
    }

    #[test]
    fn typed_bank_control_preserves_the_actual_decimal_revision_on_the_wire() {
        let request = game::GameplayUiRequest {
            expected_bank_revision: Some("42".into()),
            request: Some(Request::BankWithdraw(game::UiBankWithdrawal {
                entry_id: "7".into(),
                quantity: 1,
                noted: false,
            })),
        };
        let decoded = game::GameplayUiRequest::decode(request.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded.expected_bank_revision.as_deref(), Some("42"));
        assert!(clubscape_protocol::ui_request(&decoded).is_ok());
        let mut missing = decoded;
        missing.expected_bank_revision = None;
        assert!(clubscape_protocol::ui_request(&missing).is_err());
    }
}
