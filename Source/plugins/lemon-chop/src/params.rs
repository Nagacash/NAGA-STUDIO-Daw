// SPDX-License-Identifier: GPL-3.0-or-later
//! Persistent cue layout and host-visible controls. Source files are never modified.
use nih_plug::prelude::*;
use os_plugkit::live::LiveEdits;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{Arc, RwLock},
};
pub const PADS: usize = 32;
pub const GLOBAL: usize = 16;
pub const STRIDE: usize = 8;
pub const LEVEL: usize = 0;
pub const FILTER: usize = 1;
pub const ATTACK: usize = 2;
pub const RELEASE: usize = 3;
pub const REVERSE: usize = 4;
pub const PITCH: usize = 5;
pub const SPEED: usize = 6;
pub const OUTPUT: usize = 7;
pub const SOURCE_BPM: usize = 0;
pub const TARGET_BPM: usize = 1;
pub const SYNC: usize = 2;
pub const KEY: usize = 3;
pub const DETUNE: usize = 4;
pub const HOLD: usize = 5;
pub const POLY: usize = 6;
pub const VELOCITY: usize = 7;
pub const KEYBOARD: usize = 8;
pub const LINK: usize = 9;
pub const MASTER: usize = 10;
pub const GLIDE: usize = 11;
pub fn pad(n: usize, k: usize) -> usize {
    GLOBAL + n * STRIDE + k
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub enabled: bool,
    pub favorite: bool,
    pub name: String,
}
impl Default for Cue {
    fn default() -> Self {
        Self {
            start: 0.,
            end: 0.,
            enabled: false,
            favorite: false,
            name: String::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Document {
    pub path: String,
    pub stems: [String; 4],
    pub cues: Vec<Cue>,
    pub selected: usize,
    pub grid_start: f64,
    pub markers: Vec<f64>,
    pub key: String,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            path: String::new(),
            stems: Default::default(),
            cues: vec![Cue::default(); 32],
            selected: 0,
            grid_start: 0.,
            markers: vec![],
            key: String::new(),
        }
    }
}
impl Document {
    pub fn bpm_at(&self, time: f64, fallback: f64) -> f64 {
        for pair in self.markers.windows(2) {
            if time >= pair[0] && time < pair[1] && pair[1] - pair[0] > 0.001 {
                return 240. / (pair[1] - pair[0]);
            }
        }
        fallback
    }

    pub fn normalize(&mut self) {
        self.cues.resize(32, Cue::default());
        self.cues.truncate(32);
        self.selected = self.selected.min(31);
        for c in &mut self.cues {
            if !c.start.is_finite() || !c.end.is_finite() {
                *c = Cue::default();
            }
            c.start = c.start.max(0.);
            c.end = c.end.max(c.start);
            c.name = c.name.chars().take(64).collect();
        }
        self.markers.retain(|x| x.is_finite() && *x >= 0.);
        self.markers.sort_by(f64::total_cmp);
        self.markers.dedup();
        self.markers.truncate(128);
        if !self.grid_start.is_finite() {
            self.grid_start = 0.;
        }
    }
    pub fn autoset(
        &mut self,
        mode: usize,
        length: f64,
        bpm: f64,
        cursor: f64,
        beats: f64,
        onsets: &[f64],
        seed: u64,
    ) {
        let mut rng = seed.max(1);
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 11) as f64 / ((1u64 << 53) as f64)
        };
        let step = 60. / bpm.clamp(20., 400.) * beats.clamp(0.0625, 16.);
        let selected = self.cues[self.selected].clone();
        for i in 0..32 {
            if self.cues[i].favorite {
                continue;
            }
            let start = match mode {
                0 => onsets
                    .get(((i as f64 + next()) * onsets.len() as f64 / 32.) as usize)
                    .copied()
                    .unwrap_or(i as f64 * length / 32.),
                1 => cursor + i as f64 * step,
                2 => next() * length,
                _ => selected.start,
            };
            let end = if mode == 3 {
                selected.end
            } else {
                (start + step).min(length)
            };
            self.cues[i] = Cue {
                start: start.min(length),
                end,
                enabled: start < length && end > start,
                name: format!("Chop {:02}", i + 1),
                favorite: false,
            };
        }
    }
}
pub struct Def {
    pub id: String,
    pub name: String,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,
}
pub fn defs() -> Vec<Def> {
    let globals = [
        ("source_bpm", "Source BPM", 20., 400., 120., 0.01),
        ("tempo", "Tempo", 20., 400., 120., 0.01),
        ("sync", "Host sync", 0., 1., 1., 1.),
        ("key", "Key shift", -24., 24., 0., 1.),
        ("detune", "Detune cents", -100., 100., 0., 1.),
        ("hold", "Hold mode", 0., 1., 1., 1.),
        ("poly", "Polyphonic", 0., 1., 0., 1.),
        ("velocity", "Velocity", 0., 1., 1., 1.),
        ("keyboard", "Keyboard mode", 0., 1., 0., 1.),
        ("link", "Link pitch and speed", 0., 1., 0., 1.),
        ("master", "Output dB", -60., 12., 0., 0.1),
        ("glide", "Glide seconds", 0., 5., 0., 0.01),
        ("vocals", "Vocals", 0., 1., 1., 0.01),
        ("drums", "Drums", 0., 1., 1., 0.01),
        ("bass", "Bass", 0., 1., 1., 0.01),
        ("other", "Instruments", 0., 1., 1., 0.01),
    ];
    let mut out: Vec<_> = globals
        .into_iter()
        .map(|(id, n, min, max, d, step)| Def {
            id: id.into(),
            name: n.into(),
            min,
            max,
            default: d,
            step,
        })
        .collect();
    for i in 0..32 {
        for (id, n, min, max, d, step) in [
            ("level", "Level dB", -12., 12., 0., 0.1),
            ("filter", "Filter LP/HP", -1., 1., 0., 0.01),
            ("attack", "Attack seconds", 0., 10., 0., 0.001),
            ("release", "Release seconds", 0., 10., 0.01, 0.001),
            ("reverse", "Reverse", 0., 1., 0., 1.),
            ("pitch", "Semitones", -24., 24., 0., 1.),
            ("speed", "Speed", 0.25, 4., 1., 0.01),
            ("output", "Output", 0., 15., 0., 1.),
        ] {
            out.push(Def {
                id: format!("pad{}_{}", i + 1, id),
                name: format!("Pad {} {n}", i + 1),
                min,
                max,
                default: d,
                step,
            });
        }
    }
    out
}
pub struct ChopParams {
    pub p: Vec<FloatParam>,
    pub defs: Vec<Def>,
    pub doc: RwLock<Arc<Document>>,
    pub live: Arc<LiveEdits>,
}
impl Default for ChopParams {
    fn default() -> Self {
        let defs = defs();
        let p = defs
            .iter()
            .map(|d| {
                FloatParam::new(
                    &d.name,
                    d.default,
                    FloatRange::Linear {
                        min: d.min,
                        max: d.max,
                    },
                )
                .with_step_size(d.step)
            })
            .collect();
        Self {
            p,
            defs,
            doc: RwLock::new(Arc::new(Document::default())),
            live: Arc::default(),
        }
    }
}
impl ChopParams {
    pub fn v(&self, i: usize) -> f32 {
        self.live.value(&self.p[i])
    }
    pub fn document(&self) -> Arc<Document> {
        self.doc.read().unwrap().clone()
    }
    pub fn set_doc(&self, mut d: Document) {
        d.normalize();
        *self.doc.write().unwrap() = Arc::new(d);
    }
}
unsafe impl Params for ChopParams {
    fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
        self.p
            .iter()
            .zip(&self.defs)
            .map(|(p, d)| (d.id.clone(), p.as_ptr(), String::new()))
            .collect()
    }
    fn serialize_fields(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            (
                "chop-document".into(),
                serde_json::to_string(self.document().as_ref()).unwrap(),
            ),
            (
                "sample-path".into(),
                serde_json::to_string(&self.document().path).unwrap(),
            ),
        ])
    }
    fn deserialize_fields(&self, s: &BTreeMap<String, String>) {
        if let Some(mut d) = s
            .get("chop-document")
            .and_then(|s| serde_json::from_str::<Document>(s).ok())
        {
            if let Some(path) = s
                .get("sample-path")
                .and_then(|s| serde_json::from_str::<String>(s).ok())
                .filter(|s| !s.is_empty())
            {
                if path != d.path {
                    d.path = path;
                    d.stems = Default::default();
                }
            }
            self.set_doc(d);
        }
    }
}
