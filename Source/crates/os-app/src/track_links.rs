// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: GPL-3.0-or-later
// This file is part of Lemon Studio: https://github.com/ItsAlphaNeon/Lemon-Studio

//! Audio tracks and instrument tracks: playlist tracks linked to a mixer track or to a channel.
//!
// spec: ui/playlist.md §6c, flp/events/playlist.md

use os_model::{ChannelKind, ClipSource, TrackLink};

use crate::app::State;
use crate::playlist::{arr_mut, clips, track_info, TRACKS};

pub const ASSIGN_AUDIO: &str = "Assign to audio track(s)";
pub const UNASSIGN_AUDIO: &str = "Unassign audio track(s)";
pub const ASSIGN_INSTRUMENT: &str = "Assign to instrument track";
pub const TRACK_MODE: &str = "Playlist track mode";
pub const NOT_FOR_AUDIO: &str = "Instrument tracks are not for audio clips";

pub fn link(s: &State, t: u16) -> TrackLink {
    track_info(s, t).map_or(TrackLink::NONE, |x| x.link)
}

pub fn audio_mixer(s: &State, t: u16) -> Option<u16> {
    link(s, t).audio_mixer()
}

pub fn is_instrument(s: &State, t: u16) -> bool {
    link(s, t).kind == TrackLink::INSTRUMENT
}

fn set_link(s: &mut State, t: u16, l: TrackLink) {
    if let Some(tr) = arr_mut(s).tracks.get_mut(usize::from(t)) {
        tr.link = l;
    }
}

/// The first playlist track with no clips, no link and no name.
pub fn new_track(s: &State) -> Option<u16> {
    (0..TRACKS).find(|&t| {
        let info = track_info(s, t);
        info.is_none_or(|x| x.link.is_none() && x.name.as_deref().is_none_or(str::is_empty)) && !clips(s).iter().any(|c| c.track == t)
    })
}

pub fn audio_tracks_of(s: &State, m: u16) -> Vec<u16> {
    (0..TRACKS).filter(|&t| audio_mixer(s, t) == Some(m)).collect()
}

pub fn instrument_track_of(s: &State, ch: u16) -> Option<u16> {
    (0..TRACKS).find(|&t| link(s, t).instrument_channel() == Some(ch))
}

fn assignable(s: &State, mixers: &[u16]) -> Vec<u16> {
    mixers.iter().copied().filter(|&m| m >= 1 && m <= crate::mixer::inserts(s) && audio_tracks_of(s, m).is_empty()).collect()
}

pub fn can_assign_audio(s: &State, mixers: &[u16]) -> bool {
    !assignable(s, mixers).is_empty()
}

pub fn can_unassign_audio(s: &State, mixers: &[u16]) -> bool {
    mixers.iter().any(|&m| !audio_tracks_of(s, m).is_empty())
}

fn remember(s: &mut State, name: &'static str, mixers: &[u16]) {
    let extra: Vec<_> = mixers.iter().filter_map(|&m| crate::mixer::channel_copy(s, m).map(|c| crate::history::Snap::MixerChannel(Box::new(c)))).collect();
    if extra.is_empty() {
        crate::playlist::snapshot_as(s, name);
    } else {
        crate::playlist::snapshot_with(s, name, crate::history::Snap::Several(extra));
    }
}

pub fn assign_audio(s: &mut State, mixers: &[u16]) -> Option<String> {
    let todo = assignable(s, mixers);
    if todo.is_empty() {
        return Some("No mixer track to assign".into());
    }
    remember(s, ASSIGN_AUDIO, &[]);
    let mut made = Vec::new();
    for m in todo {
        let Some(t) = new_track(s) else { break };
        let name = crate::mixer::name(s, m);
        set_link(s, t, TrackLink::audio(m));
        arr_mut(s).tracks[usize::from(t)].name = Some(name);
        made.push((m, t));
    }
    let &(m, t) = made.last()?;
    s.playlist.track = t;
    s.piano.dirty = true;
    Some(match made.len() {
        1 => format!("Assigned {} to {}", crate::mixer::name(s, m), crate::playlist::track_name(s, t)),
        n => format!("Assigned {n} mixer tracks to new audio tracks"),
    })
}

pub fn unassign_audio(s: &mut State, mixers: &[u16]) -> Option<String> {
    let tracks: Vec<u16> = mixers.iter().flat_map(|&m| audio_tracks_of(s, m)).collect();
    if tracks.is_empty() {
        return Some("No audio track to unassign".into());
    }
    remember(s, UNASSIGN_AUDIO, &[]);
    for &t in &tracks {
        set_link(s, t, TrackLink::NONE);
    }
    s.piano.dirty = true;
    match tracks.as_slice() {
        [t] => Some(format!("Unassigned {}", crate::playlist::track_name(s, *t))),
        v => Some(format!("Unassigned {} audio tracks", v.len())),
    }
}

fn instrument_kind(k: ChannelKind) -> bool {
    !matches!(k, ChannelKind::AudioClip | ChannelKind::Automation)
}

fn instrument_rows(s: &State, rows: &[usize]) -> Vec<usize> {
    rows.iter().copied().filter(|&r| s.project.channels.get(r).is_some_and(|c| instrument_kind(c.kind) && instrument_track_of(s, c.index).is_none())).collect()
}

pub fn can_assign_instrument(s: &State, rows: &[usize]) -> bool {
    !instrument_rows(s, rows).is_empty()
}

pub fn assign_instrument(s: &mut State, rows: &[usize]) -> Option<String> {
    let todo = instrument_rows(s, rows);
    if todo.is_empty() {
        return Some("No channel to assign".into());
    }
    let mixers: Vec<u16> = todo.iter().filter_map(|&r| s.project.channels.get(r).map(|c| c.mixer_track).filter(|m| *m != 0)).collect();
    remember(s, ASSIGN_INSTRUMENT, &mixers);
    let mut made = Vec::new();
    for r in todo {
        let Some(t) = new_track(s) else { break };
        link_instrument(s, t, r);
        made.push((r, t));
    }
    let &(r, t) = made.last()?;
    s.playlist.track = t;
    s.piano.dirty = true;
    Some(match made.len() {
        1 => format!("Assigned {} to {}", crate::rack::channel_name(s, r), crate::playlist::track_name(s, t)),
        n => format!("Assigned {n} channels to new instrument tracks"),
    })
}

fn link_instrument(s: &mut State, t: u16, r: usize) {
    let Some(ch) = s.project.channels.get(r) else { return };
    let (index, color, mixer) = (ch.index, ch.color, ch.mixer_track);
    let name = crate::rack::channel_name(s, r);
    set_link(s, t, TrackLink::instrument(index));
    let tr = &mut arr_mut(s).tracks[usize::from(t)];
    tr.name = Some(name.clone());
    if color.is_some() {
        tr.color = color;
    }
    if mixer != 0 {
        if let Some(m) = crate::mixer::track_mut(s, mixer) {
            m.name = Some(name);
            if color.is_some() {
                m.color = color;
            }
        }
    }
}

/// Track mode on one playlist track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Unassigned,
    Audio(u16),
    NewAudio,
    Instrument(usize),
}

pub fn set_mode(s: &mut State, t: u16, mode: Mode) -> Option<String> {
    let mixers: Vec<u16> = match mode {
        Mode::Instrument(r) => s.project.channels.get(r).map(|c| c.mixer_track).filter(|m| *m != 0).into_iter().collect(),
        _ => Vec::new(),
    };
    remember(s, TRACK_MODE, &mixers);
    let msg = match mode {
        Mode::Unassigned => {
            set_link(s, t, TrackLink::NONE);
            format!("Unassigned {}", crate::playlist::track_name(s, t))
        }
        Mode::Audio(_) | Mode::NewAudio => {
            let m = match mode {
                Mode::Audio(m) => m,
                _ => crate::mixer::add_track(s, None),
            };
            let name = crate::mixer::name(s, m);
            set_link(s, t, TrackLink::audio(m));
            arr_mut(s).tracks[usize::from(t)].name = Some(name.clone());
            format!("Audio track {} on {name}", crate::playlist::track_name(s, t))
        }
        Mode::Instrument(r) => {
            link_instrument(s, t, r);
            format!("Instrument track {}", crate::playlist::track_name(s, t))
        }
    };
    s.piano.dirty = true;
    Some(msg)
}

pub fn mode_items(s: &State, t: u16, cmd: impl Fn(Mode) -> crate::playlist::Cmd) -> Vec<crate::playlist::PItem> {
    use crate::playlist::{Cmd, Mark, PItem};
    let l = link(s, t);
    let mut audio = vec![PItem::new("New mixer track", cmd(Mode::NewAudio)), PItem::sep()];
    audio.extend((1..=crate::mixer::inserts(s)).map(|m| PItem::new(format!("{m}. {}", crate::mixer::name(s, m)), cmd(Mode::Audio(m))).mark(Mark::Check(l.audio_mixer() == Some(m)))));
    let chans: Vec<PItem> = s.project.channels.iter().enumerate().filter(|(_, c)| instrument_kind(c.kind)).map(|(r, c)| {
        PItem::new(crate::rack::channel_name(s, r), cmd(Mode::Instrument(r))).mark(Mark::Check(l.instrument_channel() == Some(c.index)))
    }).collect();
    let existing = if chans.is_empty() { PItem::new("Use existing channel", Cmd::Nothing).off() } else { PItem::sub("Use existing channel", chans) };
    let mut items = vec![
        PItem::new("Unassigned", cmd(Mode::Unassigned)).mark(Mark::Check(l.is_none())),
        PItem::sub("Audio track", audio).mark(Mark::Check(l.kind == TrackLink::AUDIO)),
        PItem::sub("Instrument track", vec![existing]).mark(Mark::Check(l.kind == TrackLink::INSTRUMENT)),
    ];
    if let Some(m) = l.audio_mixer() {
        use crate::mixer::MCmd;
        let inputs = crate::mixer_recording::input_menu(s, m, 0.0, 0.0).items;
        let armed = s.mixer.recording.armed.contains(&m);
        items.extend([
            PItem::head("Audio track options"),
            PItem::sub("Select audio input", inputs),
            PItem::new("Arm recording", Cmd::Mixer(MCmd::SetArm(m, true))).mark(Mark::Check(armed)),
            PItem::new("Disarm recording", Cmd::Mixer(MCmd::SetArm(m, false))),
        ]);
    }
    items
}

pub fn fits(s: &State, src: ClipSource, t: u16) -> bool {
    !(is_instrument(s, t) && is_audio_source(s, src))
}

fn is_audio_source(s: &State, src: ClipSource) -> bool {
    matches!(src, ClipSource::Channel(ch) if s.project.channel(ch).is_some_and(|c| c.kind == ChannelKind::AudioClip))
}

pub fn audio_drop_track(s: &State, t: u16, a: u32, b: u32) -> u16 {
    if !is_instrument(s, t) {
        return t;
    }
    (t + 1..TRACKS).find(|&k| !is_instrument(s, k) && !clips(s).iter().any(|c| c.track == k && c.position < b && c.end() > a)).unwrap_or(t)
}

pub fn route_clips(s: &mut State, idx: &[usize]) {
    let moves: Vec<(u16, u16)> = idx.iter().filter_map(|&i| clips(s).get(i)).filter_map(|c| match c.source {
        ClipSource::Channel(ch) if is_audio_source(s, c.source) => audio_mixer(s, c.track).map(|m| (ch, m)),
        _ => None,
    }).collect();
    let max = crate::mixer::max_insert(&s.project);
    for (ch, m) in moves {
        if let Some(c) = s.project.channels.iter_mut().find(|c| c.index == ch) {
            if c.mixer_track != m && m <= max {
                c.mixer_track = m;
                s.piano.dirty = true;
            }
        }
    }
}

pub fn record_track(s: &State, m: u16, a: u32, b: u32) -> u16 {
    audio_tracks_of(s, m).into_iter().find(|&t| !clips(s).iter().any(|c| c.track == t && c.position < b && c.end() > a)).unwrap_or_else(|| crate::playlist::free_track(s, 0, a, b))
}

pub fn tracks_routed_to(s: &State, mixers: &[u16]) -> Vec<u16> {
    (0..TRACKS).filter(|&t| {
        let l = link(s, t);
        l.audio_mixer().is_some_and(|m| mixers.contains(&m)) || l.instrument_channel().and_then(|ch| s.project.channel(ch)).is_some_and(|c| mixers.contains(&c.mixer_track))
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use os_model::ClipSource;

    fn app() -> App {
        let mut a = App::new(None);
        a.state.project = crate::app::test_project();
        a
    }

    #[test]
    fn audio_and_instrument_tracks_link_and_undo() {
        let mut a = app();
        assert!(can_assign_audio(&a.state, &[2]));
        assign_audio(&mut a.state, &[2]);
        let t = audio_tracks_of(&a.state, 2)[0];
        assert_eq!(link(&a.state, t).audio_mixer(), Some(2));
        assert!(crate::history::undo(&mut a.state).is_some());
        assert!(audio_tracks_of(&a.state, 2).is_empty());

        let msg = crate::playlist::run(&mut a, crate::playlist::Cmd::TrackMode(3, Mode::Audio(4))).unwrap();
        assert!(msg.contains("Audio track"));
        assert_eq!(audio_mixer(&a.state, 3), Some(4));

        let ch = a.state.project.channels[1].index;
        assert!(can_assign_instrument(&a.state, &[1]));
        assign_instrument(&mut a.state, &[1]);
        let it = instrument_track_of(&a.state, ch).unwrap();
        assert!(is_instrument(&a.state, it));
        assert!(fits(&a.state, ClipSource::Pattern(1), it));
        a.state.project.channels[0].kind = ChannelKind::AudioClip;
        let audio = a.state.project.channels[0].index;
        assert!(!fits(&a.state, ClipSource::Channel(audio), it));
        assert!(fits(&a.state, ClipSource::Channel(ch), it));
    }
}
