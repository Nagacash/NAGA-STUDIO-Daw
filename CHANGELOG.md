# Changelog

## [Unreleased] - 2026-11-08

### Build
- macOS: run `Source/packaging/mac/build-naga-dmg.command` for a full release (app + `bundle-plugins --install` + DMG). Output: `Naga-Studio-<arch>-2026110801.dmg` in the NagaDAW folder and on Desktop.
- Build number `2026110801` in `updates.rs`.

### Added
- Mixer Look in Settings → Theme Settings, with Clean Modern, Original, Analog, and Naga Swagger; workspace textures (plain / leather / snake).
- The Mixer Look choice is stored in settings and restored on the next launch. Original stays selected until another look is chosen.
- **Channel states**: save/browse/load `.oschannel` presets (plugin window menu, Browser → Plugin presets tab → Channel presets, drag-and-drop).
- Tesla **Detect pitch regions** and playlist **Edit sample** path; `os-dsp` pitch detection.
- Playlist track mode: a track can be unassigned, an audio track linked to a mixer insert, or an instrument track linked to a channel. Audio clips on an audio track follow that insert. Instrument tracks refuse audio clips. Rename on an audio or automation clip renames its channel.
- Playlist tune: drag a detected pitch region on the clip. Each 8 px is one semitone, within two octaves. The menu nudge still works.
- Rack: swing, the mixer box, and the channel LED create automation clips. The event editor stays unavailable.

### Changed
- Clean Modern draws the Full mixer with flat rounded strips instead of the console metal.
- Analog keeps the console metal and warms the strip color.
- Theme Skins and the first two theme presets are labeled **Naga** and **Naga Classic** (replacing Lemon / Citrus in the UI). Existing `design.txt` and theme files that still say Lemon or Citrus load as before.

### Fixed
- Piano roll: Playback tool scrubs at note velocities and skips muted notes; Alt+right-click auditions; Ctrl+right-drag zoom box (upstream #305 / #349 behavior).

### Notes
- Original uses the mixer finish Naga Studio already had. Control positions, meters, routing, sends, inserts, mute, and solo are unchanged.
- The Traditional FL-style mixer layout keeps its own drawing.
- Still open vs upstream: the event editor, clip animations, lock-to-source tracks, and creating a new generator from Track mode. Undoing “New mixer track” restores the playlist link and leaves the extra insert. See `docs/lemon-studio-implementation-map.md`.
- Attribution: channel states, piano audition, audio/instrument tracks, and the rack automation targets from [Lemon Studio](https://github.com/ItsAlphaNeon/Lemon-Studio) @ `0be813c5` (GPL-3.0-or-later). Plugin user-facing vendor and about strings say Naga Studio. File headers and `lemon-*` crate paths stay.
