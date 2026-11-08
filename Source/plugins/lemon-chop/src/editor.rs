// SPDX-License-Identifier: GPL-3.0-or-later
use crate::{Shared, params::*, view};
use nih_plug::prelude::*;
use os_plugkit::{MenuDef, vals::Vals};
use os_ui::Rect;
use os_ui::kit::menu::{MenuItem, MenuStyle};
use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicIsize, AtomicU32, Ordering},
    },
};
pub struct ChopEditor {
    pub params: Arc<ChopParams>,
    pub shared: Arc<Shared>,
    /// DPI scale as f32 bits.
    pub scale: AtomicU32,
    /// The open editor window (0 = closed): parameter changes post a repaint to it.
    #[cfg_attr(not(windows), allow(dead_code))]
    hwnd: Arc<AtomicIsize>,
    /// Linux and macOS: the open editor's thread waker, for the same.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    slot: os_plugkit::gui::Slot,
}

impl ChopEditor {
    pub fn new(params: Arc<ChopParams>, shared: Arc<Shared>) -> Self {
        ChopEditor {
            params,
            shared,
            scale: AtomicU32::new(1f32.to_bits()),
            hwnd: Arc::new(AtomicIsize::new(0)),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            slot: Default::default(),
        }
    }

    fn notify(&self) {
        #[cfg(windows)]
        win32::post_changed(self.hwnd.load(Ordering::Acquire));
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        os_plugkit::gui::notify(&self.slot);
    }
}

impl Editor for ChopEditor {
    fn spawn(
        &self,
        parent: ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn Any + Send> {
        let scale = f32::from_bits(self.scale.load(Ordering::Relaxed));
        match parent {
            #[cfg(windows)]
            ParentWindowHandle::Win32Hwnd(hwnd) => Box::new(win32::open(
                hwnd,
                scale,
                self.params.clone(),
                self.shared.clone(),
                context,
                self.hwnd.clone(),
            )),
            #[cfg(target_os = "linux")]
            ParentWindowHandle::X11Window(window) => Box::new(gui::open(
                window,
                scale,
                self.params.clone(),
                self.shared.clone(),
                context,
                &self.slot,
            )),
            // macOS hosts set no scale (sizes are points): the editor paints at its window's backing scale.
            #[cfg(target_os = "macos")]
            ParentWindowHandle::AppKitNsView(view) => {
                let scale = os_plugkit::gui::backing_scale(view as usize);
                Box::new(gui::open(
                    view as usize,
                    scale,
                    self.params.clone(),
                    self.shared.clone(),
                    context,
                    &self.slot,
                ))
            }
            // Elsewhere no editor opens yet.
            _ => {
                let _ = (context, scale);
                Box::new(())
            }
        }
    }

    fn size(&self) -> (u32, u32) {
        (view::W, view::H)
    }

    fn set_scale_factor(&self, factor: f32) -> bool {
        self.scale.store(factor.to_bits(), Ordering::Relaxed);
        true
    }

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        self.notify();
    }
    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {
        self.notify();
    }
    fn param_values_changed(&self) {
        self.notify();
    }
}

#[derive(Clone, Copy, Default)]
pub struct Mods {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}
#[derive(Clone, Copy, PartialEq)]
pub enum Btn {
    Left,
    Right,
    Middle,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Button(&'static str),
    Param(usize),
    Pad(usize),
    Wave,
    Overview,
    Start,
    End,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Repaint,
    Lock,
    Menu(Target, f32, f32),
    TypeIn(String, f32, f32),
    Automate(usize),
}
#[derive(Clone, Debug)]
pub enum Cmd {
    None,
    Rename,
    Favorite,
    Delete,
    Copy,
    Paste,
    Automate(usize),
    Reset(usize),
}
#[derive(Clone)]
struct Snapshot {
    doc: Document,
    values: Vec<f32>,
}
pub struct Logic {
    pub vals: Vals,
    pub params: Arc<ChopParams>,
    pub shared: Arc<Shared>,
    pub hint: String,
    pub hosted: bool,
    pub hits: Vec<(Rect, Target)>,
    pub cursor: f64,
    pub zoom: f64,
    pub offset: f64,
    pub snap: bool,
    pub frozen: bool,
    pub grid: bool,
    pub mode: usize,
    pub beats: f64,
    pub selection: u32,
    drag: Option<(Target, f32, f32)>,
    held: Option<u8>,
    keys: [bool; 32],
    history: Vec<Snapshot>,
    future: Vec<Snapshot>,
    copy: Option<(Cue, [f32; STRIDE])>,
    analysis: u64,
    pub auto_analyse: bool,
    drop_pad: Option<usize>,
    drag_copy: bool,
}
impl Drop for Logic {
    fn drop(&mut self) {
        self.release_keys();
        self.shared.cancel.store(true, Ordering::Release);
    }
}
impl Logic {
    pub fn new(
        params: Arc<ChopParams>,
        shared: Arc<Shared>,
        ctx: Option<Arc<dyn GuiContext>>,
    ) -> Self {
        let vals = Vals::new(&*params, params.live.clone(), ctx);
        Self {
            vals,
            params,
            shared,
            hint: String::new(),
            hosted: false,
            hits: vec![],
            cursor: 0.,
            zoom: 1.,
            offset: 0.,
            snap: true,
            frozen: false,
            grid: false,
            mode: 0,
            beats: 1.,
            selection: 1,
            drag: None,
            held: None,
            keys: [false; 32],
            history: vec![],
            future: vec![],
            copy: None,
            analysis: 0,
            auto_analyse: false,
            drop_pad: None,
            drag_copy: false,
        }
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            doc: (*self.params.document()).clone(),
            values: self
                .params
                .defs
                .iter()
                .map(|d| self.vals.plain(&d.id))
                .collect(),
        }
    }
    fn remember(&mut self) {
        let s = self.snapshot();
        self.history.push(s);
        if self.history.len() > 100 {
            self.history.remove(0);
        }
        self.future.clear();
    }
    fn restore(&mut self, s: Snapshot) {
        self.params.set_doc(s.doc);
        for (i, v) in s.values.into_iter().enumerate() {
            self.set(i, v);
        }
        self.shared.stop.store(true, Ordering::Relaxed);
    }
    fn touch(&mut self) {
        let v = self.vals.norm("master");
        self.vals.edit("master", v);
    }
    pub fn set(&mut self, k: usize, v: f32) {
        let id = self.params.defs[k].id.clone();
        self.vals.edit_plain(&id, v);
    }
    pub fn poll(&mut self) {
        self.vals.expire();
        let n = self.shared.analysis_version.load(Ordering::Acquire);
        if n != self.analysis || self.auto_analyse {
            self.analysis = n;
            let a = self.shared.audio.lock().unwrap().clone();
            if let Some(a) = a.filter(|a| a.path == self.params.document().path) {
                if self.auto_analyse || !self.params.document().cues.iter().any(|c| c.enabled) {
                    self.set(SOURCE_BPM, a.bpm);
                    let mut d = (*self.params.document()).clone();
                    d.key = a.key.clone();
                    d.autoset(0, a.duration(), a.bpm as f64, 0., 1., &a.onsets, 1);
                    self.params.set_doc(d);
                    self.touch();
                    self.auto_analyse = false;
                }
            }
        }
        self.hint = self.shared.status.lock().unwrap().clone();
    }
    pub fn duration(&self) -> f64 {
        self.shared
            .audio
            .lock()
            .unwrap()
            .as_ref()
            .map(|a| a.duration())
            .unwrap_or(1.)
    }
    pub fn load(&mut self, path: String) {
        self.remember();
        self.release_keys();
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.cancel.store(true, Ordering::Release);
        self.params.set_doc(Document {
            path,
            ..Default::default()
        });
        for i in GLOBAL..self.params.p.len() {
            self.set(i, self.params.defs[i].default);
        }
        self.cursor = 0.;
        self.offset = 0.;
        self.zoom = 1.;
        self.auto_analyse = true;
        self.touch();
    }
    fn hit(&self, x: f32, y: f32) -> Option<Target> {
        self.hits
            .iter()
            .rev()
            .find(|(r, _)| r.contains(x, y))
            .map(|(_, t)| t.clone())
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some() || self.held.is_some()
    }
    fn at(&self, x: f32, overview: bool) -> f64 {
        let duration = self.duration();
        let width = duration / if overview { 1. } else { self.zoom };
        let offset = if overview { 0. } else { self.offset };
        (offset + ((x - 16.) / 1008.).clamp(0., 1.) as f64 * width).clamp(0., duration)
    }
    fn quantized(&self, t: f64) -> f64 {
        if !self.snap {
            return t;
        }
        let d = self.params.document();
        let step = 60. / d.bpm_at(t, self.params.v(SOURCE_BPM) as f64) / 4.;
        (d.grid_start + ((t - d.grid_start) / step).round() * step).clamp(0., self.duration())
    }
    pub fn down(&mut self, x: f32, y: f32, btn: Btn, m: Mods, dbl: bool) -> Vec<Act> {
        let Some(t) = self.hit(x, y) else {
            return vec![];
        };
        if btn == Btn::Right {
            return vec![Act::Menu(t, x, y)];
        }
        match t.clone() {
            Target::Param(k) => {
                if (12..16).contains(&k) && !dbl {
                    let ready = self
                        .shared
                        .audio
                        .lock()
                        .unwrap()
                        .as_ref()
                        .is_some_and(|a| a.stems.iter().all(Option::is_some));
                    if !ready {
                        self.shared.message("Choose Separate stems first");
                        return vec![Act::Repaint];
                    }
                    self.remember();
                    if m.shift {
                        for j in 12..16 {
                            self.set(j, if j == k { 1. } else { 0. });
                        }
                    } else {
                        self.set(k, if self.params.v(k) > 0. { 0. } else { 1. });
                    }
                    return vec![Act::Repaint];
                }
                if dbl {
                    return vec![Act::TypeIn(self.params.defs[k].id.clone(), x, y)];
                }
                self.remember();
                if m.alt || m.shift {
                    self.set(k, self.params.defs[k].default);
                } else if self.params.defs[k].max - self.params.defs[k].min == 1.
                    && self.params.defs[k].step == 1.
                {
                    self.set(k, 1. - self.params.v(k));
                } else {
                    self.drag = Some((t, x, y));
                    return vec![Act::Lock];
                }
            }
            Target::Pad(n) => {
                self.drop_pad = None;
                self.drag_copy = m.alt;
                if m.shift {
                    self.remember();
                    let mut d = (*self.params.document()).clone();
                    d.cues[n] = Cue::default();
                    self.params.set_doc(d);
                    self.touch();
                } else if m.ctrl {
                    self.selection ^= 1 << n;
                } else {
                    let mut d = (*self.params.document()).clone();
                    if !d.cues[n].enabled {
                        self.remember();
                        let start = self.quantized(self.cursor);
                        d.cues[n] = Cue {
                            start,
                            end: self.duration(),
                            enabled: start < self.duration(),
                            name: format!("Chop {:02}", n + 1),
                            favorite: false,
                        };
                    }
                    d.selected = n;
                    self.selection = 1 << n;
                    self.cursor = d.cues[n].start;
                    self.params.set_doc(d);
                    self.touch();
                    let note = if self.params.v(KEYBOARD) > 0.5 {
                        60
                    } else {
                        (36 + n) as u8
                    };
                    self.held = Some(note);
                    let _ = self.shared.notes.push((note, 1., true));
                    self.drag = Some((t, x, y));
                }
            }
            Target::Wave | Target::Overview => {
                self.cursor = self.at(x, t == Target::Overview);
                if t == Target::Overview {
                    self.offset = (self.cursor - self.duration() / self.zoom / 2.)
                        .clamp(0., (self.duration() - self.duration() / self.zoom).max(0.));
                }
                self.drag = Some((t, x, y));
            }
            Target::Start | Target::End => {
                self.remember();
                self.drag = Some((t, x, y));
            }
            Target::Button(name) => self.button(name, m),
        }
        vec![Act::Repaint]
    }
    pub fn delta(&mut self, _dx: f32, dy: f32, m: Mods) -> Vec<Act> {
        if let Some((Target::Param(k), _, _)) = self.drag.clone() {
            let d = &self.params.defs[k];
            let step = -(dy / 200.) * (d.max - d.min) * if m.ctrl { 0.1 } else { 1. };
            self.edit_param(k, self.params.v(k) + step);
        }
        vec![Act::Repaint]
    }
    fn edit_param(&mut self, k: usize, value: f32) {
        if k >= GLOBAL {
            let field = (k - GLOBAL) % STRIDE;
            for n in 0..32 {
                if self.selection & (1 << n) != 0 {
                    self.set(pad(n, field), value);
                }
            }
        } else {
            self.set(k, value);
        }
    }
    pub fn moved(&mut self, x: f32, y: f32, _: Mods) -> Vec<Act> {
        if let Some((t, _, _)) = self.drag.clone() {
            match t {
                Target::Pad(_) => {
                    if let Some(Target::Pad(n)) = self.hit(x, y) {
                        self.drop_pad = Some(n);
                    }
                }
                Target::Start | Target::End => {
                    let mut d = (*self.params.document()).clone();
                    let time = self.quantized(self.at(x, false));
                    let c = &mut d.cues[d.selected];
                    if t == Target::Start {
                        c.start = time.min((c.end - 0.0001).max(0.));
                    } else {
                        c.end = time.max(c.start + 0.0001).min(self.duration());
                    }
                    self.params.set_doc(d);
                    self.touch();
                }
                Target::Wave => self.cursor = self.at(x, false),
                Target::Overview => {
                    self.cursor = self.at(x, true);
                    self.offset = (self.cursor - self.duration() / self.zoom / 2.)
                        .clamp(0., (self.duration() - self.duration() / self.zoom).max(0.));
                }
                _ => {}
            }
        }
        let _ = y;
        vec![Act::Repaint]
    }
    pub fn up(&mut self, _: Btn) -> Vec<Act> {
        if let (Some((Target::Pad(source), _, _)), Some(dest)) =
            (self.drag.clone(), self.drop_pad.take())
        {
            if source != dest {
                self.remember();
                let mut d = (*self.params.document()).clone();
                let a: [f32; STRIDE] = std::array::from_fn(|k| self.params.v(pad(source, k)));
                let b: [f32; STRIDE] = std::array::from_fn(|k| self.params.v(pad(dest, k)));
                if self.drag_copy {
                    d.cues[dest] = d.cues[source].clone();
                } else {
                    d.cues.swap(source, dest);
                }
                for k in 0..STRIDE {
                    self.set(pad(dest, k), a[k]);
                    if !self.drag_copy {
                        self.set(pad(source, k), b[k]);
                    }
                }
                d.selected = dest;
                self.selection = 1 << dest;
                self.params.set_doc(d);
                self.touch();
            }
        }
        if let Some(n) = self.held.take() {
            let _ = self.shared.notes.push((n, 0., false));
        }
        self.drag = None;
        vec![Act::Repaint]
    }
    pub fn leave(&mut self) -> Vec<Act> {
        vec![Act::Repaint]
    }
    pub fn wheel(&mut self, x: f32, y: f32, n: f32, m: Mods) -> Vec<Act> {
        match self.hit(x, y) {
            Some(Target::Param(k)) => {
                self.remember();
                let d = &self.params.defs[k];
                let step = d.step.max((d.max - d.min) / 100.) * if m.ctrl { 0.1 } else { 1. };
                self.edit_param(k, self.params.v(k) + n * step);
            }
            Some(Target::Wave | Target::Start | Target::End) => {
                let anchor = self.at(x, false);
                self.zoom = (self.zoom * 2f64.powf(n as f64 * 0.5)).clamp(1., 512.);
                self.offset = (anchor - ((x - 16.) / 1008.) as f64 * self.duration() / self.zoom)
                    .clamp(0., (self.duration() - self.duration() / self.zoom).max(0.));
            }
            _ => {}
        }
        vec![Act::Repaint]
    }
    pub fn release_keys(&mut self) {
        for i in 0..32 {
            if self.keys[i] {
                let _ = self.shared.notes.push(((36 + i) as u8, 0., false));
                self.keys[i] = false;
            }
        }
        self.up(Btn::Left);
    }
    pub fn key(&mut self, key: u32, on: bool) {
        let map = b"ZXCVBNMASDFGHJKLQWERTYUI123456789";
        let upper = char::from_u32(key).unwrap_or('\0').to_ascii_uppercase() as u8;
        if let Some(i) = map.iter().position(|c| *c == upper) {
            if self.keys[i] != on {
                self.keys[i] = on;
                let _ = self
                    .shared
                    .notes
                    .push(((36 + i) as u8, if on { 1. } else { 0. }, on));
            }
        }
        if on && key == 32 {
            self.button("Stop", Mods::default());
        }
    }
    pub fn button(&mut self, name: &str, m: Mods) {
        match name {
            "Load" => {
                if let Some(p) = rfd::FileDialog::new()
                    .add_filter(
                        "Audio",
                        &["wav", "aiff", "aif", "flac", "mp3", "ogg", "m4a", "wv"],
                    )
                    .pick_file()
                {
                    self.load(p.to_string_lossy().into_owned());
                }
            }
            "Undo" => {
                if let Some(s) = self.history.pop() {
                    self.future.push(self.snapshot());
                    self.restore(s);
                }
            }
            "Redo" => {
                if let Some(s) = self.future.pop() {
                    self.history.push(self.snapshot());
                    self.restore(s);
                }
            }
            "Separate" => self.shared.separate(self.params.clone()),
            "Cancel" => self.shared.cancel.store(true, Ordering::Release),
            "Stop" => {
                self.release_keys();
                self.shared.stop.store(true, Ordering::Relaxed);
            }
            "Preview" => {
                self.shared
                    .preview_cursor
                    .store(self.cursor.to_bits(), Ordering::Relaxed);
                let _ = self.shared.notes.push((127, 1., true));
            }
            "Snap" => self.snap = !self.snap,
            "Freeze" => self.frozen = !self.frozen,
            "Grid" => self.grid = !self.grid,
            "Zoom +" => self.zoom = (self.zoom * 2.).min(512.),
            "Zoom -" => {
                self.zoom = (self.zoom / 2.).max(1.);
                self.offset = self
                    .offset
                    .min(self.duration() - self.duration() / self.zoom);
            }
            "Mode" => self.mode = (self.mode + 1) % 4,
            "Size -" => self.beats = (self.beats / 2.).max(0.0625),
            "Size +" => self.beats = (self.beats * 2.).min(16.),
            "Region <" => {
                self.cursor = (self.cursor
                    - 32. * self.beats * 60. / self.params.v(SOURCE_BPM) as f64)
                    .max(0.)
            }
            "Region >" => {
                self.cursor = (self.cursor
                    + 32. * self.beats * 60. / self.params.v(SOURCE_BPM) as f64)
                    .min(self.duration())
            }
            "Autoset" => {
                self.remember();
                let a = self.shared.audio.lock().unwrap().clone();
                if let Some(a) = a {
                    let mut d = (*self.params.document()).clone();
                    let source = d.selected;
                    let pitch = self.params.v(pad(source, PITCH));
                    d.autoset(
                        self.mode,
                        a.duration(),
                        self.params.v(SOURCE_BPM) as f64,
                        self.cursor,
                        self.beats,
                        &a.onsets,
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos() as u64,
                    );
                    if self.mode == 3 {
                        for n in 0..32 {
                            if !d.cues[n].favorite {
                                for k in 0..STRIDE {
                                    self.set(pad(n, k), self.params.v(pad(source, k)));
                                }
                                self.set(pad(n, PITCH), (pitch + n as f32 - 12.).clamp(-24., 24.));
                            }
                        }
                    }
                    self.params.set_doc(d);
                    self.touch();
                }
            }
            "All" => {
                self.selection = if self.selection == u32::MAX {
                    1 << self.params.document().selected
                } else {
                    u32::MAX
                }
            }
            "Delete" | "Favorite" | "Set start" | "Set end" | "Grid start" | "Grid marker"
            | "Clear grid" => {
                self.remember();
                let mut d = (*self.params.document()).clone();
                match name {
                    "Grid start" => {
                        d.grid_start = self.cursor;
                        d.markers = vec![self.cursor];
                    }
                    "Grid marker" => d.markers.push(self.cursor),
                    "Clear grid" => {
                        d.grid_start = 0.;
                        d.markers.clear();
                    }
                    _ => {
                        for n in 0..32 {
                            if self.selection & (1 << n) == 0 {
                                continue;
                            }
                            let c = &mut d.cues[n];
                            match name {
                                "Delete" => *c = Cue::default(),
                                "Favorite" => c.favorite = !c.favorite,
                                "Set start" => {
                                    c.start =
                                        self.quantized(self.cursor).min((c.end - 0.0001).max(0.));
                                }
                                "Set end" => {
                                    c.end = self
                                        .quantized(self.cursor)
                                        .max(c.start + 0.0001)
                                        .min(self.duration())
                                }
                                _ => {}
                            }
                        }
                    }
                }
                self.params.set_doc(d);
                self.touch();
            }
            "BPM /2" => self.set(SOURCE_BPM, self.params.v(SOURCE_BPM) / 2.),
            "BPM x2" => self.set(SOURCE_BPM, self.params.v(SOURCE_BPM) * 2.),
            "Export" => {
                let a = self.shared.audio.lock().unwrap().clone();
                if let (Some(a), Some(file)) = (
                    a,
                    rfd::FileDialog::new()
                        .set_file_name("Naga Chop.wav")
                        .save_file(),
                ) {
                    let d = self.params.document();
                    let cue = &d.cues[d.selected];
                    let start = (cue.start * a.source.rate as f64) as usize;
                    let end = ((cue.end * a.source.rate as f64) as usize).min(a.source.frames());
                    if end > start {
                        use std::io::Write;
                        let result = (|| -> std::io::Result<()> {
                            let mut f = std::io::BufWriter::new(std::fs::File::create(file)?);
                            os_engine::wav::write_f32_stereo_header(
                                &mut f,
                                a.source.rate,
                                (end - start) as u64,
                            )?;
                            for i in start..end {
                                let v = a.frame(
                                    i as f64,
                                    std::array::from_fn(|n| self.params.v(12 + n)),
                                );
                                for x in v {
                                    f.write_all(&x.to_le_bytes())?;
                                }
                            }
                            f.flush()
                        })();
                        self.shared.message(match result {
                            Ok(()) => "Selected source region exported".into(),
                            Err(e) => e.to_string(),
                        });
                    }
                }
            }
            _ => {
                let _ = m;
            }
        }
    }
    pub fn menu(&self, t: &Target) -> MenuDef<Cmd> {
        let mut menu = MenuDef::new(MenuStyle::Context);
        match t {
            Target::Pad(_) => {
                for (name, cmd) in [
                    ("Rename", Cmd::Rename),
                    ("Favorite / lock", Cmd::Favorite),
                    ("Copy pad", Cmd::Copy),
                    ("Paste pad", Cmd::Paste),
                    ("Delete", Cmd::Delete),
                ] {
                    menu.add(MenuItem::item(name), cmd);
                }
            }
            Target::Param(k) => {
                menu.add(MenuItem::item("Reset"), Cmd::Reset(*k));
                menu.add(MenuItem::item("Create automation clip"), Cmd::Automate(*k));
            }
            _ => {}
        }
        menu
    }
    pub fn pick(&mut self, t: &Target, cmd: Cmd, x: f32, y: f32) -> Vec<Act> {
        if let Target::Pad(n) = t {
            let mut d = (*self.params.document()).clone();
            d.selected = *n;
            self.params.set_doc(d);
            self.selection = 1 << n;
        }
        let n = self.params.document().selected;
        match cmd {
            Cmd::Rename => return vec![Act::TypeIn("name".into(), x, y)],
            Cmd::Favorite => self.button("Favorite", Mods::default()),
            Cmd::Delete => self.button("Delete", Mods::default()),
            Cmd::Copy => {
                self.copy = Some((
                    self.params.document().cues[n].clone(),
                    std::array::from_fn(|k| self.params.v(pad(n, k))),
                ))
            }
            Cmd::Paste => {
                if let Some((cue, values)) = self.copy.clone() {
                    self.remember();
                    let mut d = (*self.params.document()).clone();
                    d.cues[n] = cue;
                    self.params.set_doc(d);
                    for k in 0..STRIDE {
                        self.set(pad(n, k), values[k]);
                    }
                    self.touch();
                }
            }
            Cmd::Reset(k) => {
                self.remember();
                self.set(k, self.params.defs[k].default);
            }
            Cmd::Automate(i) => return vec![Act::Automate(i)],
            Cmd::None => {}
        }
        vec![Act::Repaint]
    }
    pub fn value_text(&self, id: &str) -> String {
        if id == "name" {
            let d = self.params.document();
            d.cues[d.selected].name.clone()
        } else {
            self.vals.text(id)
        }
    }
    pub fn typed(&mut self, id: &str, text: &str) {
        self.remember();
        if id == "name" {
            let mut d = (*self.params.document()).clone();
            d.cues[d.selected].name = text.chars().take(64).collect();
            self.params.set_doc(d);
            self.touch();
        } else if let Some(v) = self.vals.parse(id, text) {
            self.vals.edit(id, v);
        }
    }
}
#[cfg(windows)]
mod win32 {
    use std::ffi::c_void;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicIsize, Ordering};

    use nih_plug::prelude::GuiContext;
    use os_plugkit::win::{self as pw, CursorLock};
    use os_theme::Palette;
    use os_ui::Canvas;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::InvalidateRect;
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Controls::WM_MOUSELEAVE;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    use super::{Act, Btn, Logic, Mods};
    use crate::Shared;
    use crate::params::ChopParams;
    use crate::view;

    const TIMER: usize = 1;
    const WM_CHANGED: u32 = WM_APP + 1;

    struct Win {
        hwnd: HWND,
        canvas: Canvas,
        buf: Vec<u32>,
        scale: f32,
        /// Painted size in device px.
        size: (u32, u32),
        logic: Logic,
        palette: Palette,
        last_hint: String,
        lock: CursorLock,
        tracking: bool,
        slot: Arc<AtomicIsize>,
    }

    /// Owns the window; dropping it closes the editor.
    pub struct Handle(isize, Arc<AtomicIsize>, Arc<Shared>);

    // SAFETY: the handle is only an HWND value; the wrapper drops it on the GUI thread.
    unsafe impl Send for Handle {}

    impl Drop for Handle {
        fn drop(&mut self) {
            self.1.store(0, Ordering::Release);
            self.2.editor_open.store(false, Ordering::Relaxed);
            unsafe { DestroyWindow(self.0 as HWND) };
        }
    }

    pub fn post_changed(hwnd: isize) {
        if hwnd != 0 {
            unsafe { PostMessageW(hwnd as HWND, WM_CHANGED, 0, 0) };
        }
    }

    fn mods() -> Mods {
        let (ctrl, shift, alt) = pw::mods();
        Mods { ctrl, shift, alt }
    }

    pub fn open(
        parent: *mut c_void,
        scale: f32,
        params: Arc<ChopParams>,
        shared: Arc<Shared>,
        ctx: Arc<dyn GuiContext>,
        slot: Arc<AtomicIsize>,
    ) -> Handle {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let px = |v: u32| (v as f32 * scale).round() as u32;
        let size = (px(view::W), px(view::H));
        shared.editor_open.store(true, Ordering::Relaxed);
        unsafe {
            let class = pw::register("LemonChopView", Some(wndproc));
            let win = Box::new(Win {
                hwnd: std::ptr::null_mut(),
                canvas: Canvas::new(size.0, size.1, scale).expect("canvas"),
                buf: vec![0; (size.0 * size.1) as usize],
                scale,
                size,
                logic: Logic::new(params, shared.clone(), Some(ctx)),
                palette: os_plugkit::palette(),
                last_hint: String::new(),
                lock: CursorLock::default(),
                tracking: false,
                slot: slot.clone(),
            });
            let title = pw::wide("Naga Chop");
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                size.0 as i32,
                size.1 as i32,
                parent as HWND,
                std::ptr::null_mut(),
                GetModuleHandleW(std::ptr::null()),
                Box::into_raw(win) as *const c_void,
            );
            slot.store(hwnd as isize, Ordering::Release);
            // About 33 checks a second (edits the host confirms, the hint).
            windows_sys::Win32::UI::Shell::DragAcceptFiles(hwnd, 1);
            SetTimer(hwnd, TIMER, 30, None);
            Handle(hwnd as isize, slot, shared)
        }
    }

    unsafe fn state(hwnd: HWND) -> Option<&'static mut Win> {
        let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Win;
        if p.is_null() {
            None
        } else {
            Some(unsafe { &mut *p })
        }
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        unsafe {
            if msg == WM_NCCREATE {
                let cs = lp as *const CREATESTRUCTW;
                let w = (*cs).lpCreateParams as *mut Win;
                (*w).hwnd = hwnd;
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, w as isize);
                return DefWindowProcW(hwnd, msg, wp, lp);
            }
            if msg == WM_NCDESTROY {
                let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Win;
                if !p.is_null() {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    (*p).slot.store(0, Ordering::Release);
                    drop(Box::from_raw(p));
                }
                return DefWindowProcW(hwnd, msg, wp, lp);
            }
            let Some(w) = state(hwnd) else {
                return DefWindowProcW(hwnd, msg, wp, lp);
            };
            if msg == os_hostlink::message() {
                return w.host_link(wp, lp);
            }
            match msg {
                WM_PAINT => {
                    w.paint();
                    0
                }
                WM_DROPFILES => {
                    use windows_sys::Win32::UI::Shell::{DragFinish, DragQueryFileW};
                    let drop = wp as _;
                    let count = DragQueryFileW(drop, 0, std::ptr::null_mut(), 0);
                    let mut path = vec![0u16; count as usize + 1];
                    DragQueryFileW(drop, 0, path.as_mut_ptr(), path.len() as u32);
                    DragFinish(drop);
                    w.logic
                        .load(String::from_utf16_lossy(&path[..count as usize]));
                    w.invalidate();
                    0
                }
                WM_KEYDOWN | WM_KEYUP => {
                    if msg == WM_KEYUP || lp & (1 << 30) == 0 {
                        w.logic.key(wp as u32, msg == WM_KEYDOWN);
                    }
                    w.invalidate();
                    0
                }
                WM_KILLFOCUS => {
                    w.logic.release_keys();
                    0
                }
                WM_ERASEBKGND => 1,
                WM_TIMER | WM_CHANGED => {
                    w.poll();
                    0
                }
                WM_LBUTTONDOWN | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN | WM_RBUTTONDBLCLK
                | WM_MBUTTONDOWN => {
                    SetFocus(hwnd);
                    let (x, y) = w.mouse(lp);
                    let btn = match msg {
                        WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => Btn::Right,
                        WM_MBUTTONDOWN => Btn::Middle,
                        _ => Btn::Left,
                    };
                    let acts = w.logic.down(x, y, btn, mods(), msg == WM_LBUTTONDBLCLK);
                    if w.logic.dragging() && !acts.contains(&Act::Lock) {
                        SetCapture(hwnd);
                    }
                    w.run(acts);
                    0
                }
                WM_MOUSEMOVE => {
                    if !w.tracking {
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: hwnd,
                            dwHoverTime: 0,
                        };
                        TrackMouseEvent(&mut tme);
                        w.tracking = true;
                    }
                    if let Some((dx, dy)) = w.lock.delta() {
                        if dx != 0 || dy != 0 {
                            let acts =
                                w.logic
                                    .delta(dx as f32 / w.scale, dy as f32 / w.scale, mods());
                            w.run(acts);
                        }
                        return 0;
                    }
                    let (x, y) = w.mouse(lp);
                    let acts = w.logic.moved(x, y, mods());
                    w.run(acts);
                    0
                }
                WM_MOUSELEAVE => {
                    w.tracking = false;
                    if !w.lock.locked() && !w.logic.dragging() {
                        let acts = w.logic.leave();
                        w.run(acts);
                    }
                    0
                }
                WM_LBUTTONUP | WM_RBUTTONUP | WM_MBUTTONUP => {
                    w.lock.unlock();
                    ReleaseCapture();
                    let acts = w.logic.up(Btn::Left);
                    w.run(acts);
                    0
                }
                WM_CAPTURECHANGED => {
                    // Another window took the mouse (not this one capturing again): end the drag.
                    if w.logic.dragging() && lp as HWND != hwnd {
                        w.lock.unlock();
                        let acts = w.logic.up(Btn::Left);
                        w.run(acts);
                    }
                    0
                }
                WM_MOUSEWHEEL => {
                    // Wheel positions are screen coordinates.
                    let mut r: RECT = std::mem::zeroed();
                    GetWindowRect(hwnd, &mut r);
                    let (sx, sy) = pw::xy(lp);
                    let (x, y) = (
                        (sx - r.left) as f32 / w.scale,
                        (sy - r.top) as f32 / w.scale,
                    );
                    let notches = ((wp >> 16) & 0xffff) as u16 as i16 as f32 / 120.0;
                    let acts = w.logic.wheel(x, y, notches, mods());
                    w.run(acts);
                    0
                }
                WM_DESTROY => {
                    KillTimer(hwnd, TIMER);
                    w.lock.unlock();
                    0
                }
                _ => DefWindowProcW(hwnd, msg, wp, lp),
            }
        }
    }

    impl Win {
        fn mouse(&self, l: LPARAM) -> (f32, f32) {
            let (x, y) = pw::xy(l);
            (x as f32 / self.scale, y as f32 / self.scale)
        }

        fn screen(&self, x: f32, y: f32) -> (i32, i32) {
            pw::to_screen(self.hwnd, (x * self.scale) as i32, (y * self.scale) as i32)
        }

        fn invalidate(&self) {
            unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
        }

        /// Timer / parameter change: expire confirmed overlay values and repaint.
        fn poll(&mut self) {
            self.logic.poll();
            self.invalidate();
            if self.logic.hint != self.last_hint {
                self.last_hint = self.logic.hint.clone();
                if self.logic.hosted {
                    os_hostlink::to_host(self.hwnd as isize, os_hostlink::Cmd::Changed, 0);
                }
            }
        }

        fn run(&mut self, acts: Vec<Act>) {
            let mut repaint = false;
            for a in acts {
                match a {
                    Act::Repaint => repaint = true,
                    Act::Lock => self.lock.lock(self.hwnd),
                    Act::Menu(req, x, y) => {
                        let def = self.logic.menu(&req);
                        let (sx, sy) = self.screen(x, y);
                        if let Some(cmd) =
                            pw::popup(self.hwnd, sx, sy, &def, &self.palette, self.scale)
                        {
                            let acts = self.logic.pick(&req, cmd, x, y);
                            self.run(acts);
                        }
                        repaint = true;
                    }
                    Act::TypeIn(id, x, y) => {
                        let (sx, sy) = self.screen(x, y);
                        let cur = self.logic.value_text(&id);
                        if let Some(t) = pw::type_in(self.hwnd, sx, sy, &cur, self.scale) {
                            self.logic.typed(&id, &t);
                        }
                        repaint = true;
                    }
                    Act::Automate(i) => {
                        os_hostlink::to_host(
                            self.hwnd as isize,
                            os_hostlink::Cmd::Automate,
                            i as isize,
                        );
                    }
                }
            }
            if repaint {
                self.invalidate();
            }
        }

        /// Answers the host (spec `plugins/host-link.md`): one page, the hint line. Unknown commands return 0.
        unsafe fn host_link(&mut self, wp: WPARAM, lp: LPARAM) -> LRESULT {
            use os_hostlink::{Cmd, PageIcon};
            let Some(cmd) = Cmd::from_usize(wp) else {
                return 0;
            };
            match cmd {
                Cmd::Hello => {
                    self.logic.hosted = true;
                    self.invalidate();
                    os_hostlink::VERSION as LRESULT
                }
                Cmd::PageCount => 1,
                Cmd::PageIcon => PageIcon::Plugin as LRESULT,
                Cmd::GetPage | Cmd::SetPage => 0,
                Cmd::Metrics => os_hostlink::pack_size(view::W, view::H),
                Cmd::Hint => {
                    if lp != 0 {
                        // SAFETY: the host passes a `HintBuf` of its own process (spec: host-link.md).
                        unsafe { &mut *(lp as *mut os_hostlink::HintBuf) }.set(&self.logic.hint);
                    }
                    1
                }
                Cmd::Changed
                | Cmd::Note
                | Cmd::Automate
                | Cmd::Audio
                | Cmd::Resizable
                | Cmd::SetSize => 0,
            }
        }

        fn paint(&mut self) {
            let (hdc, ps) = pw::begin_paint(self.hwnd);
            self.logic.paint(&mut self.canvas, &self.palette);
            self.buf.resize((self.size.0 * self.size.1) as usize, 0);
            self.canvas.copy_to_xrgb(&mut self.buf);
            pw::blit(hdc, &self.buf, self.size.0, self.size.1);
            pw::end_paint(self.hwnd, &ps);
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod gui {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use nih_plug::prelude::GuiContext;
    use os_hostlink::{Cmd, PageIcon, Peer};
    use os_plugkit::gui::{self as px, Button, Input, Window};
    use os_theme::Palette;
    use os_ui::Canvas;

    use super::{Act, Btn, Logic, Mods};
    use crate::Shared;
    use crate::params::ChopParams;
    use crate::view;

    /// The editor on its GUI thread.
    struct Ed {
        win: Window,
        canvas: Canvas,
        buf: Vec<u32>,
        scale: f32,
        size: (u32, u32),
        logic: Logic,
        palette: Palette,
        last_hint: String,
        dirty: bool,
        /// The host link: the host finds the editor by the window it gave it (spec: plugins/host-link.md).
        link: Arc<Peer>,
    }

    impl Drop for Ed {
        /// The editor closes: the host no longer finds it.
        fn drop(&mut self) {
            self.link.unregister();
        }
    }

    /// Owns the editor thread; dropping it closes the editor.
    pub struct Handle(Option<px::EditorThread>, Arc<Shared>);

    impl Drop for Handle {
        fn drop(&mut self) {
            self.0.take();
            self.1.editor_open.store(false, Ordering::Relaxed);
        }
    }

    pub fn open(
        parent: px::Parent,
        scale: f32,
        params: Arc<ChopParams>,
        shared: Arc<Shared>,
        ctx: Arc<dyn GuiContext>,
        slot: &px::Slot,
    ) -> Handle {
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let size = (
            (view::W as f32 * scale).round() as u32,
            (view::H as f32 * scale).round() as u32,
        );
        let sh = shared.clone();
        // One page; joined, the editor keeps its size (it has no strip row of its own).
        let wake = slot.clone();
        let link = Peer::register(
            parent as u64,
            vec![PageIcon::Plugin],
            (view::W, view::H),
            move || px::notify(&wake),
        );
        let peer = link.clone();
        let thread = px::spawn(parent, size, slot, move |win| {
            let Ok(canvas) = Canvas::new(size.0, size.1, scale) else {
                return;
            };
            let logic = Logic::new(params, shared, Some(ctx));
            let ed = Ed {
                win,
                canvas,
                buf: Vec::new(),
                scale,
                size,
                logic,
                palette: os_plugkit::palette(),
                last_hint: String::new(),
                dirty: true,
                link: peer,
            };
            ed.run();
        });
        if thread.is_none() {
            link.unregister();
        }
        sh.editor_open.store(thread.is_some(), Ordering::Relaxed);
        Handle(thread, sh)
    }

    fn btn(b: Button) -> Btn {
        match b {
            Button::Left => Btn::Left,
            Button::Right => Btn::Right,
            Button::Middle => Btn::Middle,
        }
    }

    impl Ed {
        fn run(mut self) {
            while let Some(input) = self.win.next() {
                match input {
                    Input::Expose => self.dirty = true,
                    Input::Tick => self.poll(),
                    Input::Down { x, y, btn: b, dbl } => {
                        let (x, y) = (x / self.scale, y / self.scale);
                        let acts =
                            self.logic
                                .down(x, y, btn(b), self.mods(), dbl && b == Button::Left);
                        self.act(acts);
                    }
                    Input::Move { x, y } => {
                        let acts = self
                            .logic
                            .moved(x / self.scale, y / self.scale, self.mods());
                        self.act(acts);
                    }
                    Input::Delta { dx, dy } => {
                        let acts = self
                            .logic
                            .delta(dx / self.scale, dy / self.scale, self.mods());
                        self.act(acts);
                    }
                    Input::Up { .. } => {
                        self.win.unlock();
                        let acts = self.logic.up(Btn::Left);
                        self.act(acts);
                    }
                    Input::Leave => {
                        if !self.win.locked() && !self.logic.dragging() {
                            let acts = self.logic.leave();
                            self.act(acts);
                        }
                    }
                    Input::Wheel { x, y, notches } => {
                        let acts =
                            self.logic
                                .wheel(x / self.scale, y / self.scale, notches, self.mods());
                        self.act(acts);
                    }
                    Input::Drop { path } => {
                        self.logic.load(path);
                        self.dirty = true;
                    }
                    Input::Key { sym, .. } => {
                        self.logic.key(sym, true);
                        self.dirty = true;
                    }
                }
                if self.dirty && self.win.idle() {
                    self.paint();
                }
            }
        }

        fn mods(&self) -> Mods {
            let (ctrl, shift, alt) = self.win.mods();
            Mods { ctrl, shift, alt }
        }

        fn screen(&self, x: f32, y: f32) -> (i32, i32) {
            self.win
                .to_root((x * self.scale) as i32, (y * self.scale) as i32)
        }

        /// Timer / parameter change: expire confirmed overlay values and repaint.
        fn poll(&mut self) {
            self.logic.poll();
            self.dirty = true;
            if self.logic.hint != self.last_hint {
                self.last_hint.clone_from(&self.logic.hint);
                // The host shows the hint in its title bar (hosted mode).
                self.link.set_hint(&self.logic.hint);
                self.link.to_host(Cmd::Changed, 0);
            }
            // Joined the strip: "Create automation clip" works and the host shows the hint.
            self.logic.hosted = self.link.hosted();
        }

        fn act(&mut self, acts: Vec<Act>) {
            for a in acts {
                match a {
                    Act::Repaint => self.dirty = true,
                    Act::Lock => self.win.lock(),
                    Act::Menu(req, x, y) => {
                        let def = self.logic.menu(&req);
                        let (sx, sy) = self.screen(x, y);
                        if let Some(cmd) = self.win.popup(sx, sy, &def, &self.palette, self.scale) {
                            let acts = self.logic.pick(&req, cmd, x, y);
                            self.act(acts);
                        }
                        self.dirty = true;
                    }
                    Act::TypeIn(id, x, y) => {
                        let (sx, sy) = self.screen(x, y);
                        let cur = self.logic.value_text(&id);
                        if let Some(t) = self.win.type_in(sx, sy, &cur, self.scale) {
                            self.logic.typed(&id, &t);
                        }
                        self.dirty = true;
                    }
                    // The host makes the clip; the index is the parameter's list position.
                    Act::Automate(i) => {
                        self.link.to_host(Cmd::Automate, i as isize);
                    }
                }
            }
        }

        fn paint(&mut self) {
            self.dirty = false;
            self.logic.paint(&mut self.canvas, &self.palette);
            self.buf.resize((self.size.0 * self.size.1) as usize, 0);
            self.canvas.copy_to_xrgb(&mut self.buf);
            self.win.blit(&self.buf, self.size.0, self.size.1);
        }
    }
}

#[cfg(test)]
mod chop_tests {
    use super::*;
    #[test]
    fn chop_editor_state_gestures_favorites_and_undo() {
        let p = Arc::new(ChopParams::default());
        let shared = Arc::new(Shared::default());
        let mut l = Logic::new(p.clone(), shared, None);
        let mut d = Document::default();
        d.cues[0] = Cue {
            start: 0.,
            end: 0.5,
            enabled: true,
            name: "Kick".into(),
            ..Default::default()
        };
        p.set_doc(d);
        let mut c = os_ui::Canvas::new(view::W, view::H, 1.).unwrap();
        l.paint(&mut c, &os_plugkit::palette());
        assert_eq!(
            l.hits
                .iter()
                .filter(|(_, t)| matches!(t, Target::Pad(_)))
                .count(),
            32
        );
        for (r, _) in &l.hits {
            assert!(
                r.x >= 0.
                    && r.y >= 0.
                    && r.x + r.w <= view::W as f32
                    && r.y + r.h <= view::H as f32
            );
        }
        let center = |l: &Logic, t: Target| {
            let r = l.hits.iter().find(|(_, v)| *v == t).unwrap().0;
            (r.cx(), r.cy())
        };
        let (x, y) = center(&l, Target::Pad(0));
        l.down(x, y, Btn::Left, Mods::default(), false);
        let (x, y) = center(&l, Target::Pad(5));
        l.moved(x, y, Mods::default());
        l.up(Btn::Left);
        assert_eq!(p.document().cues[5].name, "Kick");
        assert!(!p.document().cues[0].enabled);
        l.button("Undo", Mods::default());
        assert_eq!(p.document().cues[0].name, "Kick");
        l.button("Redo", Mods::default());
        assert_eq!(p.document().cues[5].name, "Kick");
        l.typed("name", "Snare");
        assert_eq!(p.document().cues[5].name, "Snare");
        l.button("Favorite", Mods::default());
        assert!(p.document().cues[5].favorite);
        let mut d = (*p.document()).clone();
        let saved = d.cues[5].clone();
        d.autoset(2, 10., 120., 0., 1., &[], 44);
        assert_eq!(d.cues[5], saved);
        assert!(d.cues.iter().filter(|x| x.enabled).count() >= 31);
        p.set_doc(d);
        let q = ChopParams::default();
        q.deserialize_fields(&p.serialize_fields());
        assert_eq!(*q.document(), *p.document());
        l.selection = u32::MAX;
        l.edit_param(pad(5, LEVEL), -6.);
        assert!((0..32).all(|n| p.v(pad(n, LEVEL)) == -6.));
        l.key(b'Z' as u32, true);
        l.key(b'Z' as u32, true);
        l.key(b'Z' as u32, false);
        assert!(!l.keys[0]);
    }
}
