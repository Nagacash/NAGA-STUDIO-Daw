// SPDX-License-Identifier: GPL-3.0-or-later
//! Naga Chop: original slicing sampler using Lemon's audio and editor infrastructure.
#![allow(semicolon_in_expressions_from_macros)]
pub mod audio;
pub mod dsp;
pub mod editor;
pub mod params;
pub mod view;
use nih_plug::prelude::*;
use params::*;
use std::{
    num::NonZeroU32,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
    },
};
pub type Separator = fn(&str, &AtomicBool) -> Result<[String; 4], String>;
static SEPARATOR: OnceLock<Separator> = OnceLock::new();
pub fn set_separator(f: Separator) {
    let _ = SEPARATOR.set(f);
}
pub struct Shared {
    pub editor_open: AtomicBool,
    pub preview_cursor: AtomicU64,
    pub audio: Mutex<Option<Arc<audio::Audio>>>,
    pub status: Mutex<String>,
    pub notes: crossbeam_queue::ArrayQueue<(u8, f32, bool)>,
    pub sounding: AtomicU32,
    pub position: AtomicU64,
    pub tempo: AtomicU64,
    pub cancel: Arc<AtomicBool>,
    pub busy: AtomicBool,
    pub analysis_version: AtomicU64,
    pub stop: AtomicBool,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            editor_open: AtomicBool::new(false),
            preview_cursor: AtomicU64::new(0),
            audio: Mutex::new(None),
            status: Mutex::new("Load audio to start chopping".into()),
            notes: crossbeam_queue::ArrayQueue::new(256),
            sounding: AtomicU32::new(0),
            position: AtomicU64::new(0),
            tempo: AtomicU64::new(120f64.to_bits()),
            cancel: Arc::new(AtomicBool::new(false)),
            busy: AtomicBool::new(false),
            analysis_version: AtomicU64::new(0),
            stop: AtomicBool::new(false),
        }
    }
}
impl Shared {
    pub fn message(&self, s: impl Into<String>) {
        *self.status.lock().unwrap() = s.into();
    }
    pub fn separate(self: &Arc<Self>, p: Arc<ChopParams>) {
        if self.busy.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(f) = SEPARATOR.get().copied() else {
            self.message("Stem separation is available inside Lemon Studio");
            self.busy.store(false, Ordering::Release);
            return;
        };
        self.cancel.store(false, Ordering::Release);
        self.message("Separating vocals, drums, bass and instruments...");
        let shared = self.clone();
        let old = p.document();
        std::thread::spawn(move || {
            match f(&old.path, &shared.cancel) {
                Ok(stems) => {
                    if p.document().path == old.path && !shared.cancel.load(Ordering::Acquire) {
                        let mut d = (*p.document()).clone();
                        d.stems = stems;
                        p.set_doc(d);
                        shared.message("Loading separated stems...");
                    }
                }
                Err(e) => shared.message(e),
            }
            shared.busy.store(false, Ordering::Release);
        });
    }
}
pub struct LemonChop {
    pub params: Arc<ChopParams>,
    pub shared: Arc<Shared>,
    voices: Vec<dsp::Voice>,
    doc: Arc<Document>,
    audio: Option<Arc<audio::Audio>>,
    values: Vec<f32>,
    next: usize,
}
impl Default for LemonChop {
    fn default() -> Self {
        let params = Arc::new(ChopParams::default());
        let shared = Arc::new(Shared::default());
        let weak = Arc::downgrade(&shared);
        let wp = Arc::downgrade(&params);
        std::thread::spawn(move || {
            let mut loaded = (
                String::new(),
                [String::new(), String::new(), String::new(), String::new()],
            );
            loop {
                std::thread::sleep(std::time::Duration::from_millis(80));
                let (Some(sh), Some(p)) = (weak.upgrade(), wp.upgrade()) else {
                    break;
                };
                let d = p.document();
                let requested = (d.path.clone(), d.stems.clone());
                if requested == loaded {
                    continue;
                }
                loaded = requested.clone();
                if d.path.is_empty() {
                    *sh.audio.lock().unwrap() = None;
                    continue;
                }
                sh.message("Reading and analysing audio...");
                match audio::Audio::load(&d.path, &d.stems) {
                    Ok(a) => {
                        let now = p.document();
                        if now.path == requested.0 && now.stems == requested.1 {
                            *sh.audio.lock().unwrap() = Some(Arc::new(a));
                            sh.analysis_version.fetch_add(1, Ordering::Release);
                            sh.message("Ready — click pads to play; drag cue edges to trim");
                        }
                    }
                    Err(e) => {
                        *sh.audio.lock().unwrap() = None;
                        sh.message(e);
                    }
                }
            }
        });
        Self {
            doc: params.document(),
            values: vec![0.; GLOBAL + 32 * STRIDE],
            params,
            shared,
            voices: Vec::new(),
            audio: None,
            next: 0,
        }
    }
}
impl Drop for LemonChop {
    fn drop(&mut self) {
        self.shared.cancel.store(true, Ordering::Release);
    }
}
impl LemonChop {
    fn note(&mut self, note: u8, channel: u8, velocity: f32, on: bool) {
        if !on || velocity == 0. {
            for v in &mut self.voices {
                v.off(note, channel);
            }
            return;
        }
        if note == 127 {
            if let Some(audio) = &self.audio {
                for v in &mut self.voices {
                    v.stop();
                }
                let mut values = [0.; GLOBAL + 32 * STRIDE];
                values.copy_from_slice(&self.values);
                values[HOLD] = 0.;
                values[KEYBOARD] = 0.;
                values[pad(0, REVERSE)] = 0.;
                values[pad(0, PITCH)] = 0.;
                values[pad(0, SPEED)] = 1.;
                self.voices[0].start(
                    audio.clone(),
                    &self.doc,
                    &values,
                    0,
                    127,
                    0,
                    1.,
                    f64::from_bits(self.shared.tempo.load(Ordering::Relaxed)),
                    Some(f64::from_bits(
                        self.shared.preview_cursor.load(Ordering::Relaxed),
                    )),
                );
            }
            return;
        }
        let n = if self.values[KEYBOARD] > 0.5 {
            self.doc.selected
        } else {
            let Some(i) = note.checked_sub(36).filter(|i| *i < 32) else {
                return;
            };
            i as usize
        };
        let Some(audio) = &self.audio else { return };
        if audio.path != self.doc.path {
            return;
        }
        if self.values[POLY] < 0.5 && self.values[KEYBOARD] > 0.5 && self.values[GLIDE] > 0. {
            if let Some(v) = self.voices.iter_mut().find(|v| v.active) {
                v.glide_to(
                    note,
                    self.values[GLIDE],
                    std::array::from_fn(|i| self.values[12 + i]),
                );
                return;
            }
        }
        if self.values[POLY] < 0.5 {
            for v in &mut self.voices {
                v.stop();
            }
        }
        let i = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or(self.next % 32);
        self.next = (i + 1) % 32;
        self.voices[i].start(
            audio.clone(),
            &self.doc,
            &self.values,
            n,
            note,
            channel,
            velocity,
            f64::from_bits(self.shared.tempo.load(Ordering::Relaxed)),
            None,
        );
    }
}
impl Plugin for LemonChop {
    const NAME: &'static str = "Naga Chop";
    const VENDOR: &'static str = "Naga Studio";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None,
        main_output_channels: NonZeroU32::new(2),
        aux_output_ports: &[NonZeroU32::new(2).unwrap(); 15],
        ..AudioIOLayout::const_default()
    }];
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;
    type SysExMessage = ();
    type BackgroundTask = ();
    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }
    fn editor(&mut self, _: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        Some(Box::new(editor::ChopEditor::new(
            self.params.clone(),
            self.shared.clone(),
        )))
    }
    fn initialize(
        &mut self,
        _: &AudioIOLayout,
        c: &BufferConfig,
        _: &mut impl InitContext<Self>,
    ) -> bool {
        self.voices = (0..32)
            .map(|_| dsp::Voice::new(c.sample_rate as u32))
            .collect();
        let d = self.params.document();
        if !d.path.is_empty() {
            match audio::Audio::load(&d.path, &d.stems) {
                Ok(a) => {
                    let a = Arc::new(a);
                    *self.shared.audio.lock().unwrap() = Some(a.clone());
                    self.audio = Some(a);
                }
                Err(e) => self.shared.message(e),
            }
        }
        true
    }
    fn reset(&mut self) {
        for v in &mut self.voices {
            v.stop();
        }
        while self.shared.notes.pop().is_some() {}
        self.shared.sounding.store(0, Ordering::Relaxed);
    }
    fn process(
        &mut self,
        b: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        ctx: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if let Ok(d) = self.params.doc.try_read() {
            self.doc = d.clone();
        }
        if let Ok(a) = self.shared.audio.try_lock() {
            self.audio = a.clone();
        }
        for (i, v) in self.values.iter_mut().enumerate() {
            *v = self.params.v(i);
        }
        if let Some(bpm) = ctx.transport().tempo {
            self.shared.tempo.store(bpm.to_bits(), Ordering::Relaxed);
        }
        if self.shared.stop.swap(false, Ordering::Relaxed) {
            self.reset();
        }
        let tempo = f64::from_bits(self.shared.tempo.load(Ordering::Relaxed));
        for v in &mut self.voices {
            v.update(&self.values, tempo);
        }
        while let Some((n, v, on)) = self.shared.notes.pop() {
            self.note(n, 0, v, on);
        }
        for o in aux.outputs.iter_mut() {
            for channel in o.as_slice() {
                channel.fill(0.);
            }
        }
        let gains = std::array::from_fn(|i| self.values[12 + i]);
        let master = 10f32.powf(self.values[MASTER] / 20.);
        let mut event = ctx.next_event();
        let out = b.as_slice();
        if out.len() < 2 {
            return ProcessStatus::Normal;
        }
        for frame in 0..out[0].len() {
            while event.as_ref().is_some_and(|e| e.timing() as usize <= frame) {
                match event.take().unwrap() {
                    NoteEvent::NoteOn {
                        note,
                        channel,
                        velocity,
                        ..
                    } => self.note(note, channel, velocity, true),
                    NoteEvent::NoteOff { note, channel, .. } => self.note(note, channel, 0., false),
                    _ => {}
                }
                event = ctx.next_event();
            }
            let mut sum = [0f32; 2];
            for v in &mut self.voices {
                let sample = v.tick(gains);
                if !cfg!(feature = "linked") && v.output > 0 && v.output <= aux.outputs.len() {
                    let dst = aux.outputs[v.output - 1].as_slice();
                    if dst.len() >= 2 && frame < dst[0].len() {
                        dst[0][frame] += sample[0] * master;
                        dst[1][frame] += sample[1] * master;
                    }
                } else {
                    sum[0] += sample[0];
                    sum[1] += sample[1];
                }
            }
            out[0][frame] = sum[0] * master;
            out[1][frame] = sum[1] * master;
        }
        let mut sounding = 0u32;
        for v in &self.voices {
            if v.active {
                sounding |= 1 << v.pad;
                self.shared
                    .position
                    .store(v.position().to_bits(), Ordering::Relaxed);
            }
        }
        self.shared.sounding.store(sounding, Ordering::Relaxed);
        ProcessStatus::KeepAlive
    }
}
impl ClapPlugin for LemonChop {
    const CLAP_ID: &'static str = "org.lemonstudio.lemon-chop";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("32-pad sample chopping and stem instrument");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Sampler,
        ClapFeature::Stereo,
    ];
}
impl Vst3Plugin for LemonChop {
    const VST3_CLASS_ID: [u8; 16] = *b"LemonStudioChop1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Sampler];
}
#[cfg(not(feature = "linked"))]
nih_export_clap!(LemonChop);
#[cfg(not(feature = "linked"))]
nih_export_vst3!(LemonChop);
os_plugkit::linked_clap_entry!(LemonChop);
