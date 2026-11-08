# Upstream Attribution

## Lemon Studio

- Repository: [https://github.com/ItsAlphaNeon/Lemon-Studio](https://github.com/ItsAlphaNeon/Lemon-Studio)
- License: GPL-3.0-or-later
- Upstream commit/version used: `0be813c5dd60c73090847b5889b15a88c66a915a` (2026-11-07)
- Date integrated: 2026-11-08
- Reference clone: `../reference/lemon-studio` (not part of the Naga source tree, not modified)

Naga DAW's `Source/` tree is already GPL-3.0-or-later and is a customized Lemon Studio snapshot. That license match is what allows GPL code from the reference clone to be ported later.

**Branding:** Lemon Studio is upstream only. User-facing strings, settings, dialogs, installers, and help must say **Naga Studio** / **Naga DAW**, not Lemon Studio. Porting code does not revert product naming, logos, or in-app credits to Lemon. Theme skins show **Naga** / **Naga Classic**; `design.txt` and `LemonStudioColors=` still use the legacy `Lemon` / `Citrus` file tokens for compatibility.

## Reused components

| Upstream path | License | Reuse type | Naga DAW destination | Modifications | Notes |
|---|---|---|---|---|---|
| `crates/os-app/src/views/mixer.rs` (Lemon card branch already present in Naga) | GPL-3.0-or-later | Adapted in place | `Source/crates/os-app/src/views/mixer.rs` | Clean Modern selects the existing rounded-card painters. Original still follows the previous metal rule. | No new upstream file was copied for Mixer Look. Upstream has skins named Lemon and Citrus, not Clean Modern, Original, or Analog. |
| `crates/os-app/src/views/mixer/analog.rs` (Naga painter, GPL header) | GPL-3.0-or-later | Adapted in place | same path | Analog look warms the strip color passed into the existing painters. | This file is not in the upstream clone. |
| `crates/os-theme/src/design.rs` | GPL-3.0-or-later | Reference only | unchanged (`Source/crates/os-theme/src/design.rs` matches upstream) | None | Whole-app skins stay as they are. Mixer Look does not switch the app skin. |

## Third-party components

| Component | License | Used for | Notes |
|---|---|---|---|
| SnapTune | AGPL-3.0 | Live vocal correction already on mixer strips | Not used for Mixer Look. Notice already in `Source/crates/os-dsp/vendor/snaptune/LICENSE` and `Source/licenses/SnapTune-AGPL-3.0.txt`. |
| Signalsmith Stretch | MIT | Existing time-stretch | Not used by this update. Candidate for a later clip pitch editor. |

## Compliance notes

- [x] GPL license text preserved where required (`Source/LICENSE` unchanged)
- [x] Copyright notices preserved (no upstream file copied in this update)
- [x] SPDX identifiers preserved where present
- [x] Attribution added
- [x] Third-party notices reviewed (`Source/THIRD-PARTY-NOTICES.txt`, SnapTune AGPL notice). No new third-party component added.
- [x] Naga DAW distribution/license compatibility confirmed for this update: `Source/` is GPL-3.0-or-later
