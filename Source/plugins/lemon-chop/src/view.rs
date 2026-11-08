// SPDX-License-Identifier: GPL-3.0-or-later
//! Lemon Pith surfaces, knobs and typography; original waveform/pad layout.
use crate::{
    editor::{Logic, Target},
    params::*,
};
use os_theme::design::lemon as brand;
use os_ui::{
    Canvas, Color, Font, Rect,
    kit::{
        State,
        lemon::{self, plug},
    },
};
use std::sync::atomic::Ordering;
pub const W: u32 = 1040;
pub const H: u32 = 748;
fn color(n: usize) -> Color {
    [
        0xd6b85c, 0x87b68a, 0x6fabb9, 0xb99ad0, 0xcf8a79, 0x88a9ca, 0xb8be73, 0xc58da9,
    ]
    .map(Color::rgb)[n % 8]
}
impl Logic {
    fn button_view(&mut self, c: &mut Canvas, r: Rect, label: &str, id: &'static str, on: bool) {
        let ink = lemon::plug_button(
            c,
            r,
            5.,
            State {
                on,
                ..State::NORMAL
            },
        );
        c.text_center(Font::new(13), r, ink, label);
        self.hits.push((r, Target::Button(id)));
    }
    fn param_view(&mut self, c: &mut Canvas, r: Rect, k: usize, label: &str) {
        let v = self.params.v(k);
        let toggle = self.params.defs[k].step == 1. && self.params.defs[k].max == 1.;
        let ink = lemon::plug_button(
            c,
            r,
            5.,
            State {
                on: toggle && v > 0.5,
                ..State::NORMAL
            },
        );
        let text = if toggle {
            label.to_string()
        } else {
            format!("{label}  {v:.2}")
        };
        c.text_center(Font::new(13), r, ink, &text);
        self.hits.push((r, Target::Param(k)));
    }
    pub fn paint(&mut self, c: &mut Canvas, t: &os_theme::Palette) {
        self.poll();
        self.hits.clear();
        c.begin_frame(None);
        c.clear(plug::BG);
        c.text(Font::new(25).bold(), 16., 12., brand::MAIN, "LEMON CHOP");
        c.text(Font::new(12), 186., 21., plug::DIM, "SLICE • STEM • PLAY");
        self.button_view(c, Rect::new(674., 12., 64., 30.), "Load", "Load", false);
        self.button_view(c, Rect::new(744., 12., 64., 30.), "Undo", "Undo", false);
        self.button_view(c, Rect::new(814., 12., 64., 30.), "Redo", "Redo", false);
        self.button_view(c, Rect::new(884., 12., 64., 30.), "Export", "Export", false);
        self.button_view(c, Rect::new(954., 12., 70., 30.), "Stop", "Stop", false);
        let d = self.params.document();
        let a = self.shared.audio.lock().unwrap().clone();
        lemon::plug_panel(c, Rect::new(16., 52., 1008., 62.));
        let file = std::path::Path::new(&d.path)
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Drop an audio file here, or choose Load".into());
        c.text_clip(Font::new(17), 28., 61., 355., brand::TEXT, &file);
        let description = a
            .as_ref()
            .map(|a| {
                format!(
                    "{:.2} seconds   •   {} Hz   •   {} (estimated)",
                    a.duration(),
                    a.source.rate,
                    a.key
                )
            })
            .unwrap_or_else(|| {
                "32 pads • MIDI notes 36–67 • keyboard mode rooted at C4 (60)".into()
            });
        c.text_clip(Font::new(12), 28., 86., 355., plug::DIM, &description);
        self.param_view(c, Rect::new(398., 61., 116., 34.), SOURCE_BPM, "BPM");
        self.param_view(c, Rect::new(520., 61., 70., 34.), SYNC, "Sync");
        self.param_view(c, Rect::new(596., 61., 116., 34.), TARGET_BPM, "Tempo");
        self.param_view(c, Rect::new(718., 61., 98., 34.), KEY, "Key");
        self.param_view(c, Rect::new(822., 61., 98., 34.), DETUNE, "Cents");
        self.param_view(c, Rect::new(926., 61., 86., 34.), LINK, "Link");
        for (i, (label, id, on)) in [
            ("Snap", "Snap", self.snap),
            ("Freeze", "Freeze", self.frozen),
            ("Grid", "Grid", self.grid),
            ("−", "Zoom -", false),
            ("+", "Zoom +", false),
            ("Set start", "Set start", false),
            ("Set end", "Set end", false),
            ("Preview", "Preview", false),
        ]
        .into_iter()
        .enumerate()
        {
            self.button_view(
                c,
                Rect::new(16. + i as f32 * 80., 122., 74., 25.),
                label,
                id,
                on,
            );
        }
        if self.grid {
            for (i, (label, id)) in [
                ("Grid start", "Grid start"),
                ("Bar anchor", "Grid marker"),
                ("Clear", "Clear grid"),
                ("BPM /2", "BPM /2"),
                ("BPM x2", "BPM x2"),
            ]
            .into_iter()
            .enumerate()
            {
                self.button_view(
                    c,
                    Rect::new(660. + i as f32 * 73., 122., 68., 25.),
                    label,
                    id,
                    false,
                );
            }
        }
        let wave = Rect::new(16., 156., 1008., 166.);
        let overview = Rect::new(16., 329., 1008., 42.);
        lemon::plug_well(c, wave, 6.);
        lemon::plug_well(c, overview, 5.);
        self.hits.push((wave, Target::Wave));
        self.hits.push((overview, Target::Overview));
        if let Some(a) = &a {
            let duration = a.duration();
            let span = duration / self.zoom;
            self.offset = self.offset.min((duration - span).max(0.));
            if !self.frozen && self.shared.sounding.load(Ordering::Relaxed) != 0 {
                let pos = f64::from_bits(self.shared.position.load(Ordering::Relaxed));
                if pos < self.offset || pos > self.offset + span {
                    self.offset = (pos - span * 0.25).clamp(0., (duration - span).max(0.));
                }
            }
            for (r, offset, width) in [(wave, self.offset, span), (overview, 0., duration)] {
                for x in 0..1006 {
                    let f = (offset + x as f64 / 1006. * width) / duration;
                    let j = (f * 4096.) as usize;
                    if let Some(&(amp, high)) = a.peaks.get(j) {
                        let col = brand::MAIN.mix(Color::rgb(0x6daebd), high);
                        let h = (amp * r.h * 0.43).min(r.h * 0.46);
                        c.line(
                            r.x + 1. + x as f32,
                            r.cy() - h,
                            r.x + 1. + x as f32,
                            r.cy() + h,
                            1.,
                            col,
                        );
                    }
                }
                for (i, cue) in d.cues.iter().enumerate().filter(|(_, c)| c.enabled) {
                    let x = r.x + ((cue.start - offset) / width) as f32 * r.w;
                    if x >= r.x && x <= r.x + r.w {
                        c.line(x, r.y + 2., x, r.y + r.h - 2., 1., color(i));
                    }
                }
            }
            let beat = 60. / self.params.v(SOURCE_BPM) as f64;
            let first = ((self.offset - d.grid_start) / beat).ceil() as i64;
            for b in first..first + 2048 {
                let time = d.grid_start + b as f64 * beat;
                if time > self.offset + span {
                    break;
                }
                let x = wave.x + ((time - self.offset) / span) as f32 * wave.w;
                if x >= wave.x && x <= wave.x + wave.w {
                    c.line(
                        x,
                        wave.y,
                        x,
                        wave.y + if b % 4 == 0 { wave.h } else { 12. },
                        1.,
                        plug::TICK.with_alpha(140),
                    );
                }
            }
            let cue = &d.cues[d.selected];
            if cue.enabled {
                let x1 = wave.x + ((cue.start - self.offset) / span) as f32 * wave.w;
                let x2 = wave.x + ((cue.end - self.offset) / span) as f32 * wave.w;
                let l = x1.clamp(wave.x, wave.x + wave.w);
                let r = x2.clamp(wave.x, wave.x + wave.w);
                if r > l {
                    c.fill_rect(
                        Rect::new(l, wave.y + 1., r - l, wave.h - 2.),
                        color(d.selected).with_alpha(35),
                    );
                }
                for (x, target) in [(x1, Target::Start), (x2, Target::End)] {
                    if (wave.x..=wave.x + wave.w).contains(&x) {
                        c.line(x, wave.y, x, wave.y + wave.h, 2., color(d.selected));
                        c.fill_rrect(
                            Rect::new(
                                (x - 5.).clamp(wave.x, wave.x + wave.w - 10.),
                                wave.y,
                                10.,
                                18.,
                            ),
                            2.,
                            color(d.selected),
                        );
                        self.hits
                            .push((Rect::new(x - 6., wave.y, 12., wave.h), target));
                    }
                }
            }
            let pos = if self.shared.sounding.load(Ordering::Relaxed) != 0 {
                f64::from_bits(self.shared.position.load(Ordering::Relaxed))
            } else {
                self.cursor
            };
            let x = wave.x + ((pos - self.offset) / span) as f32 * wave.w;
            if x >= wave.x && x <= wave.x + wave.w {
                c.line(x, wave.y, x, wave.y + wave.h, 1., brand::TEXT);
            }
            let x = overview.x + (self.offset / duration) as f32 * overview.w;
            let width = (span / duration) as f32 * overview.w;
            c.stroke_rrect(
                Rect::new(x, overview.y, width, overview.h),
                3.,
                1.,
                brand::TEXT,
            );
        } else {
            c.text_center(
                Font::new(21),
                wave,
                plug::DIM,
                "LOAD AUDIO • FIND YOUR CHOPS • MAKE IT YOURS",
            );
        }
        for (i, label) in ["Vocals", "Drums", "Bass", "Instruments"]
            .into_iter()
            .enumerate()
        {
            self.param_view(
                c,
                Rect::new(16. + i as f32 * 119., 381., 113., 27.),
                12 + i,
                label,
            );
        }
        let busy = self.shared.busy.load(Ordering::Relaxed);
        self.button_view(
            c,
            Rect::new(498., 381., 124., 27.),
            if busy {
                "Cancel separation"
            } else {
                "Separate stems"
            },
            if busy { "Cancel" } else { "Separate" },
            busy,
        );
        self.param_view(
            c,
            Rect::new(636., 381., 70., 27.),
            HOLD,
            if self.params.v(HOLD) > 0.5 {
                "Hold"
            } else {
                "Trigger"
            },
        );
        self.param_view(
            c,
            Rect::new(712., 381., 70., 27.),
            POLY,
            if self.params.v(POLY) > 0.5 {
                "Poly"
            } else {
                "Mono"
            },
        );
        self.param_view(c, Rect::new(788., 381., 90., 27.), KEYBOARD, "Keyboard");
        self.param_view(c, Rect::new(884., 381., 80., 27.), VELOCITY, "Velocity");
        lemon::plug_panel(c, Rect::new(16., 418., 1008., 91.));
        let n = d.selected;
        for (i, (field, label)) in [
            (LEVEL, "LEVEL"),
            (FILTER, "FILTER"),
            (ATTACK, "ATTACK"),
            (RELEASE, "RELEASE"),
            (PITCH, "PITCH"),
            (SPEED, "SPEED"),
        ]
        .into_iter()
        .enumerate()
        {
            let k = pad(n, field);
            let cx = 62. + i as f32 * 122.;
            let v = self.params.v(k);
            let def = &self.params.defs[k];
            let norm = (v - def.min) / (def.max - def.min);
            c.fill_circle(cx, 459., 20., plug::FIELD);
            c.stroke_circle(cx, 459., 19., 2., color(n));
            let angle = (225. - 270. * norm).to_radians();
            c.line(
                cx,
                459.,
                cx + angle.cos() * 14.,
                459. - angle.sin() * 14.,
                2.,
                brand::TEXT,
            );
            c.text_center(
                Font::new(11),
                Rect::new(cx - 49., 423., 98., 18.),
                plug::DIM,
                label,
            );
            let text = match field {
                ATTACK | RELEASE => format!("{:.0} ms", v * 1000.),
                SPEED => format!("{v:.2}x"),
                _ => format!("{v:.2}"),
            };
            c.text_center(
                Font::new(12),
                Rect::new(cx - 49., 483., 98., 20.),
                brand::TEXT,
                &text,
            );
            self.hits
                .push((Rect::new(cx - 45., 423., 90., 80.), Target::Param(k)));
        }
        self.param_view(
            c,
            Rect::new(760., 432., 84., 28.),
            pad(n, REVERSE),
            "Reverse",
        );
        self.button_view(
            c,
            Rect::new(852., 432., 78., 28.),
            "Favorite",
            "Favorite",
            d.cues[n].favorite,
        );
        self.param_view(c, Rect::new(938., 432., 74., 28.), MASTER, "dB");
        if cfg!(feature = "linked") {
            c.text_center(
                Font::new(12),
                Rect::new(760., 469., 126., 26.),
                plug::DIM,
                "Main stereo output",
            );
        } else {
            self.param_view(
                c,
                Rect::new(760., 469., 126., 26.),
                pad(n, OUTPUT),
                "Output",
            );
        }
        self.param_view(c, Rect::new(892., 469., 120., 26.), GLIDE, "Glide");
        let mode = ["Find transients", "Beat slicer", "Random", "Key shift pads"][self.mode];
        self.button_view(c, Rect::new(16., 519., 150., 28.), mode, "Mode", false);
        self.button_view(
            c,
            Rect::new(172., 519., 84., 28.),
            "Autoset",
            "Autoset",
            true,
        );
        self.button_view(c, Rect::new(264., 519., 42., 28.), "−", "Size -", false);
        c.text_center(
            Font::new(12),
            Rect::new(309., 519., 78., 28.),
            brand::TEXT,
            &format!("{} beats", self.beats),
        );
        self.button_view(c, Rect::new(390., 519., 42., 28.), "+", "Size +", false);
        self.button_view(
            c,
            Rect::new(440., 519., 70., 28.),
            "Region <",
            "Region <",
            false,
        );
        self.button_view(
            c,
            Rect::new(516., 519., 70., 28.),
            "Region >",
            "Region >",
            false,
        );
        self.button_view(
            c,
            Rect::new(846., 519., 84., 28.),
            "Select all",
            "All",
            self.selection == u32::MAX,
        );
        self.button_view(
            c,
            Rect::new(938., 519., 86., 28.),
            "Delete",
            "Delete",
            false,
        );
        let sounding = self.shared.sounding.load(Ordering::Relaxed);
        for i in 0..32 {
            let cue = &d.cues[i];
            let r = Rect::new(
                16. + (i % 8) as f32 * 127.,
                558. + (i / 8) as f32 * 39.,
                119.,
                32.,
            );
            let col = color(i);
            lemon::outlined(
                c,
                r,
                5.,
                if self.selection & (1 << i) != 0 {
                    col
                } else {
                    plug::PANEL_LINE
                },
                if sounding & (1 << i) != 0 {
                    col
                } else {
                    plug::FIELD
                },
            );
            if cue.enabled {
                c.fill_rrect(Rect::new(r.x + 2., r.y + 4., 3., 24.), 1., col);
            }
            let ink = if sounding & (1 << i) != 0 {
                plug::WELL
            } else {
                brand::TEXT
            };
            c.text(
                Font::new(11),
                r.x + 10.,
                r.y + 4.,
                ink,
                &format!("{:02}{}", i + 1, if cue.favorite { " *" } else { "" }),
            );
            c.text_clip(
                Font::new(12),
                r.x + 35.,
                r.y + 4.,
                78.,
                ink,
                if cue.enabled { &cue.name } else { "Empty" },
            );
            c.text(
                Font::new(10),
                r.x + 35.,
                r.y + 18.,
                plug::DIM,
                &format!("MIDI {}", 36 + i),
            );
            self.hits.push((r, Target::Pad(i)));
        }
        c.text_clip(Font::new(12), 16., 724., 1008., plug::DIM, &self.hint);
        let _ = t;
    }
}
