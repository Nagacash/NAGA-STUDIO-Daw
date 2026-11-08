# Lemon Studio integration map

**Product:** Naga Studio / Naga DAW. **Upstream reference:** Lemon Studio (GPL). When porting, keep Naga user-facing copy, assets, and branding. Do not rename the app, settings labels, or help back to Lemon Studio unless the string is an immutable FL or file-format identifier.

Upstream: https://github.com/ItsAlphaNeon/Lemon-Studio
Reference clone: `../reference/lemon-studio` (read-only)
Upstream commit: `0be813c5dd60c73090847b5889b15a88c66a915a` (2026-11-07, "Merge pull request #388 from ItsAlphaNeon/old-names-citrus-mods")
Naga source: `Source/` (GPL-3.0-or-later, already a customized Lemon Studio tree)
Date: 2026-11-08

License gate: Naga DAW's `Source/LICENSE` is GPL-3.0. Porting GPL Lemon Studio code into it is compatible. SnapTune, already vendored under `Source/crates/os-dsp/vendor/snaptune`, is AGPL-3.0 and stays a separate notice. No Lemon assets, icons, or branding were copied.

The trees have diverged. Of the comparable files, 482 match, 297 differ, 192 exist only upstream, and 670 exist only in Naga (branding, SnapTune, Lemon Wave, Lemon Chop, packaging, vendor). Do not replace Naga files wholesale.

## 1. Mixer appearance themes

| | |
|---|---|
| Upstream | `crates/os-theme/src/design.rs` (skins Lemon and Citrus), `crates/os-app/src/views/settings.rs`, `crates/os-app/src/views/mixer.rs` |
| Naga destination | `Source/crates/os-app/src/prefs.rs`, `views/settings.rs`, `views/mixer.rs`, existing `views/mixer/analog.rs` |
| Strategy | Adapt. Upstream has no Clean Modern / Original / Analog mixer looks. It has two whole-app skins. |
| Dependencies | Existing prefs combo store (`settings.txt`). Existing analog painters and the in-tree Lemon card branch. |
| Risks | Naga's Full mixer already paints with `analog.rs`. A new look must not move hit targets. |
| Changes | Settings → Theme → Mixer Look. Original is the default and keeps today's metal rule. Clean Modern turns those painters off and uses the rounded card finish. Analog keeps the metal and warms the strip color. Traditional FL-style layout is a separate painter and is unchanged. |
| Tests | `values_and_defaults` (missing and unknown values stay Original). `mixer_look_original_keeps_todays_metal_rule`. `declarations_match_the_settings_pages` requires the combo on a settings page. |
| License | No new upstream file copied. Behavior reuses code already in Naga under GPL-3.0-or-later, copyright ItsAlphaNeon where those headers exist. |

Status: implemented in this update. Added **Naga Swagger** (analog metal + snake-leather mixer back) and **Empty workspace** textures (plain / leather / snake).

## 1b. Naga swagger textures (Naga-only)

| | |
|---|---|
| Destination | `views/naga_texture.rs`, prefs `workspace_texture`, `mixer_look` = Naga Swagger |
| Strategy | Procedural leather + snake scales (no Lemon upstream). Optional PNG tile via `.flstheme` or Kling art in `assets/themes/`. |
| Status | implemented 2026-11-08 |

## 2. In-playlist vocal tuning editor

| | |
|---|---|
| Upstream | `crates/os-dsp/src/pitch.rs` (YIN note regions, used by Tesla "Detect pitch regions"). No Melodyne-style editor. |
| Naga destination | `crates/os-dsp/src/pitch.rs`, Tesla `Detect pitch regions`, playlist **Edit sample**. SnapTune live strips remain separate (AGPL). |
| Strategy | Port `pitch.rs` for detection. Adapt playlist clip state, undo, and Tesla/piano-roll interaction patterns for the editor. Do not treat SnapTune's live processor as the clip editor. |
| Dependencies | Clip model in `os-model`, playlist view, history snapshots, project save in `os-flp`. Pitch shift of a region can use the existing Signalsmith stretcher (MIT) rather than expanding AGPL SnapTune. |
| Risks | Note editing that survives cut, move, duplicate, undo, and reload is a new schema. YIN regions are coarse (about 70 ms). This stays experimental. |
| Changes | Detection, the Pitch regions menu, and a vertical drag on the region drawn on the clip. Each 8 px is one semitone, clamped to two octaves. The stretch writes `{stem}.naga-tune.wav`. |
| Tests | Menu nudge stays one undo step. The drag snapshots the channel sample once at press. |
| License | `pitch.rs` is GPL-3.0-or-later, copyright ItsAlphaNeon. |

Status: drag-to-tune is in the playlist clip (2026-11-08). This is Naga's region editor, not a Melodyne port. Upstream has no in-playlist editor.

## 3. Lemon EQ upgrade

| | |
|---|---|
| Upstream | `plugins/lemon-parametric-eq-2/` (`view.rs`, `editor.rs`, `params.rs`, `analyzer.rs`), `crates/os-engine/src/effects/eq/` |
| Naga destination | Same paths under `Source/`. Both trees already have 7 bands, the graph, right-click band menus, and a spectrum analyzer. `analyzer.rs` is identical. |
| Strategy | Port the small upstream delta in `view.rs`, `editor.rs`, `params.rs`, and `lib.rs` after a line diff, keeping Naga names and SnapTune-unrelated call sites. Dynamic EQ is not in upstream. |
| Dependencies | `os-engine` EQ coefficients, host automation, project slot state. |
| Risks | Naga `params.rs` is slightly larger than upstream. A blind replace can drop Naga state fields. |
| Changes | Naga keeps `routed::RoutedEq` and the Naga names. The upstream delta that mattered was the hollow steep-order marks; `order_mark` already draws them. |
| Tests | `steep_order_marks_are_hollow` in `plugins/lemon-parametric-eq-2/src/view.rs`. |
| License | GPL-3.0-or-later. Keep file headers. |

Status: Naga EQ stays the routed superset (2026-11-08). Order-mark drawing is covered by the restored test. Do not replace the plugin files.

## 4. Playlist improvements

| | |
|---|---|
| Upstream | `crates/os-app/src/playlist.rs` (5956 lines), `views/playlist.rs` (1833). Recent commits include clip preview playback, Alt+right click, automation drawing, clip animations, audio and instrument tracks. |
| Naga destination | `playlist.rs` (5377 lines), `views/playlist.rs` (1410). |
| Strategy | Port selected commits. Do not replace the files. Naga's copies have local edits. |
| Dependencies | History, project model, clip rendering. |
| Risks | A full file replace drops Naga playlist behavior. |
| Changes | Audio tracks and instrument tracks (`track_links.rs`, FLP event 238 bytes 49–60). Rename on an audio or automation clip opens the channel name. Automation curves already draw their own points. |
| Tests | `audio_and_instrument_tracks_link_and_undo`. |
| License | GPL-3.0-or-later. Record each ported upstream commit in the attribution table (reference repo, not a git push). |

Status: audio and instrument tracks ported (2026-11-08). Not ported: clip animations, lock-to-source tracks, and Track mode → new generator. Undoing “New mixer track” restores the playlist link and leaves the extra insert.

## 5. Piano roll improvements

| | |
|---|---|
| Upstream | `piano.rs` and `views/piano.rs`. Recent commit `934d59d`: notes sound at their velocity, the Playback tool holds them, Ctrl/Alt with the right button do not delete. |
| Naga destination | Nearly the same size (1349 and 784–785 lines) but the bytes differ. |
| Strategy | Diff and port the audition fixes first. Leave the rest of the tool set in place. |
| Dependencies | Note preview audio path. |
| Risks | Small diffs can still be Naga-specific branding or calls. |
| Changes | `scrub_keys` skips muted notes, Playback scrub uses per-note velocity, Alt/Ctrl+right-click keep the notes. Naga harmony and the event clipboard stay. |
| Tests | `the_playback_tool_holds_what_its_line_is_on`, `right_modifiers_in_the_piano_roll_keep_the_notes`, `playback_scrub_skips_muted_notes`. |
| License | GPL-3.0-or-later. |

Status: **934d59d audition fixes ported** (2026-11-08), including muted notes.

## 6. Channel rack improvements

| | |
|---|---|
| Upstream | `views/rack.rs`, `views/channel_win.rs` (181 lines), `channel_states.rs` (223 lines, absent from Naga). Commit `3ba61e7` automates swing, the mixer box, and the LED. |
| Naga destination | `views/rack.rs`, `views/channel_win.rs` (82 lines), `channel_states.rs`. |
| Strategy | Port `channel_states.rs`, then the automation commit. Keep Naga instrument integrations. |
| Dependencies | Channel model, undo, pattern data. |
| Risks | New module may pull app.rs call sites that collide with Naga edits in `app.rs`. |
| Changes | Channel states, plus swing / mixer-box / LED automation targets and Create automation clip. “Edit events” stays grey: Naga has no event editor. |
| Tests | `channel_states::tests::save_add_and_browse`, `swing_mixer_box_and_led_make_automation_clips`. |
| License | GPL-3.0-or-later. |

Status: **channel states ported** (2026-11-08). Swing, the mixer box, and the LED create automation clips. The event editor is not ported.
