"""Authored evidence scopes, mandatory cases and pre-candidate comparison policy."""

import re


def wiki(title):
    return "wiki." + re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")


def update(number, extension="png"):
    return f"Grid Master Rewards, Poll & New Player Improvements ({number}).{extension}"


def music_update(number, extension="png"):
    return f"Music System Improvements & Deadman Tweaks ({number}).{extension}"


TUTORIAL_GROUPS = [
    ("appearance", "Customization", ["Player customisation interface.png"], [], "starting-house"),
    ("experience", "Customization", [update(4)], [929], "starting-house"),
    ("guide_greeting settings_open guide_settings starting_exit", "Gielinor Guide",
     ["Gielinor Guide chathead.png", "Display options interface.png", update(9)],
     [162, 231, 614], "starting-house"),
    ("survival_greeting inventory_open catch_shrimp skills_open survival_tools cut_logs light_fire cook_shrimp survival_exit",
     "Survival Expert", ["Survival Expert chathead.png", "Inventory tab.png", "Skills tab.png",
                         update(7), update(10, "jpg"), update(11, "gif")],
     [149, 320, 614, 231], "survival-coast"),
    ("chef_entry chef_greeting make_dough bake_bread chef_exit run_toggle", "Master Chef",
     ["Tutorial Island kitchen.png", "Master Chef chathead.png", "Run energy orb.gif",
      "Choose Option use interface.png"], [149, 162, 231, 614], None),
    ("quest_entry quest_greeting journal_open quest_explanation quest_ladder", "Quest Guide",
     ["Quest house.png", "Quest Guide chathead.png", "Quest tab.png", update(5), update(6)],
     [119, 399, 231, 614], None),
    ("mining_greeting mine_first mine_second smelt_bronze mining_hammer anvil_open smith_dagger mining_exit",
     "Mining Instructor", ["Tutorial Island Mine.png", "Mining Instructor chathead.png",
                          "Smelting.gif", "Smithing and Silver Crafting Interfaces (1).png"],
     [149, 231, 614], None),
    ("combat_greeting equipment_open equipment_stats_open equip_dagger melee_supply equip_melee combat_open enter_rat_pen melee_rat leave_rat_pen ranged_supply equip_ranged ranged_rat combat_exit",
     "Combat Instructor", ["Tutorial Island Mine.png", "Combat Instructor chathead.png",
                           "Worn Equipment tab.png", "Equipment Stats interface.png",
                           "Attack style tab.png", "CombatStyles Slash Sword.png"],
     [84, 85, 149, 231, 593, 614], None),
    ("bank_open bank_close poll_inspect account_entry account_greeting account_open account_explanation account_exit",
     "Banking tutorial", ["Tutorial Island bank.png", "Bank interface.png",
                          "Account Guide chathead.png", "Account Management tab.png",
                          update(13), update(14)],
     [12, 15, 182, 231, 614], None),
    ("chapel_entry prayer_greeting prayer_open prayer_explanation chapel_exit",
     "Prayer Tutorial", ["Tutorial Island chapel.png", "Brother Brace chathead.png",
                         "Prayer tab.png", "Activating quick prayers.gif"],
     [231, 541, 614], None),
    ("magic_entry magic_greeting magic_open magic_supply wind_strike departure_offer departure_confirmation home_teleport teleport_channel mainland",
     "Magic Instructor", ["Magic building.png", "Magic Instructor chathead.png",
                           "Standard spells.png", "Wind Strike.gif", "Home Teleport.gif",
                           "Learning the Ropes reward scroll.png", update(12)],
     [149, 153, 218, 219, 231, 614], None),
]


# Each tuple is an independently named Section 30 requirement, not a media count.
EXTRA_CASES = [
    ("entry.title", "Title / welcome", "title", [], [0], []),
    ("entry.login", "Login form and focus", "title", [], [2], []),
    ("entry.terms", "Source terms / privacy / EULA", "title", [], [12], []),
    ("entry.loading", "Original startup loading", "loading", [], [], []),
    ("entry.connecting", "Connecting and asset progress", "proposal", ["connecting"], [], []),
    ("entry.authentication_failure", "Invalid authentication feedback", "title", [], [3], []),
    ("entry.reconnect", "Connection lost / reconnect / expired session", "public",
     ["Connection lost.png"], [], [162]),
    ("entry.registration", "ClubScape account registration", "proposal", ["registration"], [], []),
    ("entry.registration_rejected", "Registration rejection with preserved fields", "proposal",
     ["registration-rejected"], [], []),
    ("entry.capability_error", "Required WebGPU / desktop browser feedback", "proposal",
     ["capability"], [], []),
    ("entry.runtime_error", "Unexpected startup/runtime failure", "proposal", ["runtime-error"], [], []),
    ("entry.unavailable", "Preserved out-of-scope control / unavailable feedback", "proposal",
     ["unavailable"], [], []),
    ("entry.branding", "ClubScape name within source composition", "proposal", ["branding"], [], []),
    ("hud.resizable_classic", "Full Resizable - Classic frame", "public",
     [music_update(4), "Minimap.png", "Chat Interface.png"], [], [161, 162]),
    ("hud.minimap", "Minimap / compass / markers / run / prayer orbs", "public",
     ["Minimap.png", "Run energy orb.gif", "Activating quick prayers.gif"], [], [161]),
    ("hud.chat", "Chat channels / input / focus / scrolling", "public",
     ["Chat Interface.png", "Clear Private Chat & More (1).png"], [], [162]),
    ("hud.tabs_unlocks", "All tabs and progressive tutorial locks", "public",
     [music_update(4), update(9), "Quest tab.png"], [], [161, 614]),
    ("hud.context_menu", "Default / right / modified clicks and item-use menus", "public",
     ["Choose Option interface.png", "Choose Option monster interface.png",
      "Choose Option object interface.png", "Choose Option weapon interface.png",
      "Choose Option use interface.png", "Choose Option spell interface.png"], [], []),
    ("ui.inventory", "Inventory / stacks / selection / drag / rejected action", "public",
     ["Inventory tab.png", "Choose Option use interface.png"], [], [149]),
    ("ui.equipment", "Equipment and all functional slots / stats", "public",
     ["Worn Equipment tab.png", "Equipment Stats interface.png"], [], [84, 85]),
    ("ui.skills", "24 skills / XP / current level-up notification", "public",
     ["Skills tab.png", "Cow Boss and Skill Guide & Level-Up Changes (5).png",
      "Cow Boss and Skill Guide & Level-Up Changes (8).gif",
      "Cow Boss and Skill Guide & Level-Up Changes (10).png"], [], [320]),
    ("ui.combat", "Melee / ranged styles / auto-retaliate", "public",
     ["Attack style tab.png", "CombatStyles Slash Sword.png"], [], [593]),
    ("ui.prayer", "Prayer / unavailable prayers / active state", "public",
     ["Prayer tab.png", "Activating quick prayers.gif"], [], [541]),
    ("ui.magic", "Spellbook filtering / Wind Strike / Home Teleport", "public",
     ["Standard spells.png", "Choose Option spell interface.png", "Wind Strike.gif"], [], [218]),
    ("ui.bank", "Bank / tabs / search / placeholders / notes / amounts", "public",
     ["Bank interface.png", "Bank settings menu.png"], [], [12, 15, 192, 213]),
    ("ui.shop", "Lumbridge shop / stock / buy / sell / amount / rejection", "public",
     ["Lumbridge General Store stock.png"], [], [300, 301]),
    ("ui.smithing", "Bronze anvil selection / unavailable products", "public",
     ["Smithing and Silver Crafting Interfaces (1).png"], [], []),
    ("ui.dialogue", "NPC chathead / instruction box / choices / continue", "public",
     [update(4), update(9), update(12)], [], [162, 219, 231]),
    ("ui.quests", "Quest list / journal / not-started / active / complete", "public",
     ["Quest tab.png", update(5), update(6)], [], [119, 399]),
    ("ui.quest_rewards", "Learning the Ropes and Cook's Assistant rewards", "public",
     ["Learning the Ropes reward scroll.png", "Cook's Assistant reward scroll.png"], [], [153]),
    ("ui.settings", "Settings / classic layout selection / controls", "public",
     ["Settings interface.png", "Display options interface.png"], [], [161]),
    ("ui.account_poll_logout", "Account links / source poll / logout controls", "public",
     ["Account Management tab.png", update(13), update(14), "Display name interface.png"], [], [182, 558, 929]),
    ("ui.death_recovery", "First death / office / kept items / grave recovery", "public",
     ["Death's Office.png", "Death Changes (1).png", "Death Changes (3).png",
      "Death interface.png", "Death Changes (7).png", "Dying animation.gif"], [], [4, 669, 670, 671]),
    ("world.tutorial_house", "Tutorial starting-house scenery", "scene",
     ["tutorial-starting-house"], [], []),
    ("world.tutorial_coast", "Tutorial survival-coast scenery", "scene",
     ["tutorial-survival-coast"], [], []),
    ("world.lumbridge_arrival", "Lumbridge arrival / castle / experience branches", "scene",
     ["lumbridge-castle-plaza"], [], []),
    ("world.lumbridge_bridge", "Lumbridge river and bridge", "scene",
     ["lumbridge-river-bridge"], [], []),
    ("world.cooks_route", "Cook's kitchen, farms and three-floor windmill route", "scene",
     ["lumbridge-windmill-route"], [], []),
    ("model.tree", "Ordinary source tree / orientations / chopping state", "model", ["tree"], [], []),
    ("model.goblin", "Ordinary source goblin idle / walk / combat identity", "model", ["goblin"], [], []),
    ("model.penguin", "Penguin base / motion / equipment-fit adaptation", "model", ["penguin"], [], []),
    ("audio.music.title", "Scape Main / startup gesture", "audio", ["music.0"], [], []),
    ("audio.music.tutorial_surface", "Newbie Melody / tutorial surface", "audio", ["music.62"], [], []),
    ("audio.music.tutorial_dungeon", "Scape Cave / tutorial underground", "audio", ["music.144"], [], []),
    ("audio.music.lumbridge", "Harmony / Lumbridge", "audio", ["music.76"], [], []),
    ("audio.music.farms", "Autumn Voyage / farms", "audio", ["music.2"], [], []),
    ("audio.effects.gathering", "Mining / chopping / fire / net fishing sounds", "audio", ["gathering"], [], []),
    ("audio.effects.production", "Cooking / dough / smelting / smithing sounds", "audio", ["production"], [], []),
    ("audio.effects.combat", "Sword / bow / goblin / rat / magic / food / death sounds", "audio", ["combat"], [], []),
    ("audio.effects.interaction", "Equipment / item / prayer / bank / shop sounds", "audio", ["interaction"], [], []),
    ("audio.effects.ambient", "Original source ambient objects / positional fields", "audio", ["ambient"], [], []),
    ("audio.jingles", "Tutorial / Cook's Assistant / level-up jingle selection", "audio", ["jingles"], [], []),
    ("audio.controls", "2026 volume / mute / playlists / single / area / shuffle", "audio", ["controls"], [], [239]),
    ("audio.transitions", "Source music transition recording / loop boundaries", "audio", ["transitions"], [], [239]),
    ("audio.reconnect", "Audio focus / gesture / reconnect duplication prevention", "audio", ["reconnect"], [], []),
]


NUMERIC_PROFILES = {
    "native_sprite_font": {
        "alignment_translation_px": 0, "scale": 1,
        "glyph_advance_error_px": 0, "glyph_mask_different_pixels": 0,
        "font_ascent_error_px": 0, "sprite_opaque_different_pixels": 0,
        "sprite_alpha_error": 0, "palette_channel_error": 0,
        "justification": "These are unchanged source bitmaps and 257-byte font metrics; resampling, "
                         "substitute fonts, palette shifts or altered advances have no raster excuse.",
    },
    "native_scene_model": {
        "alignment_translation_px": 0, "scale": 1,
        "source_frame_index_error": 0, "camera_input_error_source_units": 0,
        "nonedge_max_channel_error_8bit": 2, "nonedge_mean_abs_channel_error_8bit": 0.35,
        "silhouette_hausdorff_px": 1, "edge_band_radius_px": 1,
        "edge_band_changed_fraction_of_full_image_max": 0.005,
        "missing_geometry_pixels": 0, "source_vertex_count_difference": 0,
        "source_triangle_count_difference": 0, "animation_phase_error_client_cycles_max": 1,
        "justification": "Small integer/float raster-edge differences only. Compute the 1-pixel band "
                         "from SOURCE edges before comparison. All geometry, textures, interior "
                         "shading and background stay in the comparison; never mask a creature or scenery.",
    },
    "public_lossless_ui": {
        "alignment_translation_px": 0, "scale": 1,
        "matched_native_component_pixel_error": 0,
        "text_advance_error_px": 0, "layout_anchor_error_px": 1,
        "flat_region_max_channel_error_8bit": 0,
        "edge_band_radius_px": 1, "edge_mean_abs_channel_error_8bit_max": 1,
        "changed_pixel_fraction_max": 0.002,
        "justification": "Only explicitly reconciled unchanged components are normative. The complete "
                         "supplied panel remains checked. Dynamic text/values use independent pinned source "
                         "oracles and native glyphs, not another screenshot or an exclusion mask. "
                         "A historical crop does not establish a current full-frame layout.",
    },
    "public_lossy_motion": {
        "alignment_translation_px": 0, "scale": 1,
        "layout_anchor_error_px": 1, "mean_abs_channel_error_8bit_max": 2,
        "channel_error_p99_8bit_max": 12, "silhouette_hausdorff_px_max": 1,
        "source_recording_time_error_seconds_max": 0.02,
        "current_native_sprite_font_error": 0,
        "justification": "JPEG/H.264/GIF evidence has encoded raster/temporal limitations, not permission "
                         "to redesign. These limits apply only to a matched, known-scale reference. "
                         "Unknown camera, image scaling or build blocks calibrated acceptance; do not "
                         "rescale a candidate to minimize its difference.",
    },
    "owner_composition": {
        "unchanged_source_region_different_pixels": 0, "frame_size_error_px": 0,
        "source_font_advance_error_px": 0, "source_sprite_scale": 1,
        "proposed_control_anchor_error_px": 0,
        "glyph_clip_pixels": 0, "text_or_control_overlap_pixels": 0,
        "justification": "Only named text/branding/control additions are proposals. Reused source frames "
                         "and all unchanged pixels must remain exact. The proposal is not an approved "
                         "baseline, a source screenshot or a game client.",
    },
    "audio_source": {
        "source_file_hash_difference": 0, "decoded_pcm_hash_difference": 0,
        "browser_float_abs_error_max": 0.000023,
        "additional_clip_samples": 0, "proposed_gain_error_db_max": 0.25,
        "proposed_known_frame_event_onset_error_ms_max": 20,
        "proposed_channel_transition_overlap_error_ms_max": 20,
        "duplicate_playing_sources": 0, "unbound_trigger_timing_tolerance_ms": None,
        "justification": "Lossless current native PCM is exact. The float allowance bounds the already "
                         "measured Chrome positive-int16 mapping (0.00002282857894897461). "
                         "20 ms is one proposed client cycle, NOT an observed packet delay or music fade. "
                         "Unknown trigger identities/timing must be resolved, not assigned zero or guessed.",
    },
    "penguin_adaptation": {
        "unchanged_source_frame_vertex_error": 0,
        "unchanged_source_palette_error": 0, "source_scale_numerator": 75,
        "source_scale_denominator": 128, "proposed_attachment_gap_source_units_max": 2,
        "proposed_unintended_equipment_penetration_source_units_max": 1,
        "missing_functional_equipment_slots": 0, "animation_frame_omissions": 0,
        "justification": "The original NPC is a reuse-first OWNER-REVIEW candidate, not an approved player. "
                         "Attachment/contact thresholds are separate proposals and do not relax ordinary "
                         "tree/goblin/scene fidelity or permit deleting functional equipment slots.",
    },
    "dynamic_state_text": {
        "text_codepoint_mismatches": 0, "dynamic_value_mismatches": 0,
        "glyph_advance_error_px": 0, "glyph_mask_different_pixels": 0,
        "source_font_id_mismatches": 0, "source_colour_run_mismatches": 0,
        "strikethrough_flag_mismatches": 0, "required_panel_pixel_coverage": 1.0,
        "unverified_font_or_background_substitutions": 0,
        "justification": "Reuse source-backed panel geometry, source pixels/components and native fonts. "
                         "Validate data-only changes from pinned transcripts/contracts, never from the "
                         "candidate's first screenshot. Every dynamic rectangle includes checked source "
                         "background; no entire text box, slot grid or panel is ignored.",
    },
    "native_hud": {
        "alignment_translation_px": 0, "scale": 1,
        "native_widget_bounds_error_px": 0, "native_attachment_mismatches": 0,
        "unchanged_ui_pixel_error": 0, "glyph_advance_error_px": 0,
        "source_sprite_or_font_substitutions": 0, "unchecked_frame_pixel_fraction": 0,
        "world_profile": "native_scene_model",
        "dynamic_text_profile": "dynamic_state_text",
        "justification": "Compare the full original frame at its declared controlled state. UI regions retain "
                         "exact source pixels/geometry/attachments; scene rasterization uses the existing narrow "
                         "native-scene profile, never a scenery mask. Synthetic fixture text is usable for that "
                         "fixture's pixel replay only; journey dialogue comes from pinned transcript/native-font oracles.",
    },
}


DISPOSITION_GROUPS = [
    ("ui_panel_crop", "Current widgets/components must be reconciled; upload date is not capture/build date.",
     ["Inventory tab.png", "Worn Equipment tab.png", "Equipment Stats interface.png", "Skills tab.png",
      "Attack style tab.png", "CombatStyles Slash Sword.png", "Prayer tab.png", "Standard spells.png",
      "Bank interface.png", "Bank settings menu.png", "Lumbridge General Store stock.png", "Quest tab.png",
      "Audio options interface.png", "Music tab.png", "Settings interface.png", "Display options interface.png",
      "Chat Interface.png", "Player customisation interface.png", "Display name interface.png",
      "Account Management tab.png", "Adventure Paths interface.png", "Minimap.png",
      "Learning the Ropes reward scroll.png", "Cook's Assistant reward scroll.png",
      "Choose Option interface.png", "Choose Option monster interface.png", "Choose Option object interface.png",
      "Choose Option spell interface.png", "Choose Option use interface.png", "Choose Option weapon interface.png"]),
    ("ui_control_sprite", "A small control icon, NOT the equipment-statistics or items-kept-on-death panel.",
     ["Equipment Stats.png", "Items kept on death.png"]),
    ("isolated_npc_component", "Chathead illustration only, not a dialogue frame, verified animation phase or spawn.",
     ["Gielinor Guide chathead.png", "Survival Expert chathead.png", "Master Chef chathead.png",
      "Quest Guide chathead.png", "Mining Instructor chathead.png", "Combat Instructor chathead.png",
      "Account Guide chathead.png", "Brother Brace chathead.png", "Magic Instructor chathead.png",
      "Cook (Lumbridge) chathead.png"]),
    ("scene_or_object_illustration", "Camera/build/lighting/plugins are unknown. Supplementary location/shape "
     "evidence only; original runtime scenes remain the calibrated geometric inputs.",
     ["Starting house.png", "Tutorial Island kitchen.png", "Quest house.png", "Tutorial Island bank.png",
      "Tutorial Island chapel.png", "Magic building.png", "Tutorial Island Mine.png", "Lumbridge.png",
      "Mill Lane Mill.png", "Mill Lane Mill (interior, ground floor).png",
      "Mill Lane Mill (interior, 1st floor).png", "Mill Lane Mill (interior, 2nd floor).png",
      "Cook's Assistant.png", "Death's Office.png", "Grave.png"]),
    ("animation_crop", "Encoded GIF frames/delays are exact file facts, not native wall-clock/sequence calibration.",
     ["Home Teleport.gif", "Smelting.gif", "Wind Strike.gif", "Dying animation.gif",
      "Run energy orb.gif", "Activating quick prayers.gif"]),
    ("historical_variant", "Historical component/style only. Do not use old controls/state rules for build 240.",
     ["Skills level-up.gif", "Polls interface.png", "Death interface.png",
      "Death Changes (1).png", "Death Changes (3).png", "Death Changes (4).png",
      "Death Changes (5).png", "Death Changes (7).png",
      "Clear Private Chat & More (1).png", "Clear Private Chat & More (2).png"]),
    ("nonbaseline_layout_or_crop", "Not a complete stock Resizable - Classic frame. Modern/fixed/scaled/plugin "
     "context must not become the target. Retained as an explicitly rejected full-frame candidate.",
     ["RuneLite client.png", "Resizeable Mode (1).png", "Resizeable Mode (2).png",
      "Default login screen.png", "Connection lost.png",
      "Halloween 2018 Event and Tutorial Island Improvements (7).png"]),
    ("official_tutorial_component", "Official released October 2025 example, not a build-240 run. The old "
     "'Tutorial Island' quest title in (5)/(6) is superseded by Learning the Ropes; yellow scene outlines "
     "in (7)/(9)/(10)/(11) are explicitly Official Client effects, not proven stock Java/injected effects.",
     [update(n) for n in (4, 5, 6, 7, 8, 9, 12, 13, 14)] + [update(10, "jpg"), update(11, "gif")]),
    ("official_music_component", "Released February 2026 music controls. (2) is a full recording with "
     "Resizable Modern/scaled UI, not the requested Classic full-frame. It proves an example transition, "
     "not every region boundary, mixer level, trigger or Java-client fade.",
     [music_update(n) for n in (3, 4, 5, 6)] + [music_update(1, "gif"), music_update(2, "mp4")]),
    ("official_current_levelup_component", "March 2026 level-up changes supersede the 2020 GIF. Chatbox "
     "notifications and skill highlights are source examples; skill-specific content still comes from current rules.",
     ["Cow Boss and Skill Guide & Level-Up Changes (5).png",
      "Cow Boss and Skill Guide & Level-Up Changes (8).gif",
      "Cow Boss and Skill Guide & Level-Up Changes (10).png"]),
    ("official_production_component", "2019 source anvil/silver crafting frames; only the bronze anvil "
     "panel is a journey requirement. Current unavailable products and quantities are not inferred from this upload.",
     ["Smithing and Silver Crafting Interfaces (1).png", "Smithing and Silver Crafting Interfaces (2).png"]),
]


def dispositions():
    result = {}
    for kind, note, titles in DISPOSITION_GROUPS:
        for title in titles:
            key = wiki(title)
            if key in result:
                raise ValueError(f"Duplicate authored disposition: {key}")
            result[key] = {"media_scope": kind, "compatibility_note": note,
                           "full_resizable_classic_frame": False}
    return result
