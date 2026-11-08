// SPDX-License-Identifier: GPL-3.0-or-later
//! Naga Studio finish: leather and snake-scale fills for empty workspace and mixer backplates.
//! Empty workspace → Naga snake draws `themes/pics/naga-snake-leather.png` when that file is found.
//! These fills are the fallback, and the mixer backplate.

use os_ui::{Color, Font, Rect};

use crate::app::Ui;

const LEATHER_DARK: Color = Color::rgb(0x14110e);
const LEATHER_MID: Color = Color::rgb(0x2a231c);
const LEATHER_HI: Color = Color::rgb(0x3d342b);
const SCALE_EDGE: Color = Color::rgb(0x1a1512);
const SCALE_GLOW: Color = Color::rgb(0x4a6b4f);
const SCALE_SHINE: Color = Color::rgb(0xc4a35a);

/// Fills `r` with dark leather grain.
pub fn leather(ui: &mut Ui, r: Rect) {
    if r.w <= 0.0 || r.h <= 0.0 {
        return;
    }
    ui.c.fill_rect_gradient(r, LEATHER_HI.mix(LEATHER_MID, 0.35), LEATHER_DARK, 0.0);
    let step = 7.0_f32;
    let rows = ((r.h / step).ceil() as i32).min(512);
    for i in 0..rows {
        let y = r.y + i as f32 * step;
        let wobble = ((i * 17 + 3) % 5) as f32 * 0.4;
        let alpha = (12 + (i % 4) * 4) as u8;
        ui.c.line(
            r.x,
            y + wobble,
            r.right(),
            y + wobble + 0.3,
            1.0,
            Color::rgb(0x000000).with_alpha(alpha),
        );
        if i % 3 == 0 {
            ui.c.line(
                r.x,
                y + 2.0,
                r.right(),
                y + 2.0,
                0.5,
                LEATHER_HI.with_alpha(8),
            );
        }
    }
    // Tooled border inset.
    let inset = r.inset(1.5);
    ui.c.line(inset.x, inset.y, inset.right(), inset.y, 1.0, SCALE_SHINE.with_alpha(28));
    ui.c.line(inset.x, inset.bottom(), inset.right(), inset.bottom(), 1.0, SCALE_EDGE.with_alpha(90));
}

/// Leather plus overlapping snake scales (low contrast so it reads as texture, not wallpaper).
pub fn snake(ui: &mut Ui, r: Rect) {
    leather(ui, r);
    let cols = ((r.w / 14.0).ceil() as i32).min(400);
    let rows = ((r.h / 11.0).ceil() as i32).min(400);
    for row in 0..rows {
        let stagger = if row % 2 == 0 { 0.0 } else { 7.0 };
        for col in 0..cols {
            let cx = r.x + stagger + col as f32 * 14.0 + 7.0;
            let cy = r.y + row as f32 * 11.0 + 5.5;
            if cx < r.x - 7.0 || cx > r.right() + 7.0 || cy < r.y || cy > r.bottom() {
                continue;
            }
            let pick = (row * 31 + col * 17) % 7;
            let glow = SCALE_GLOW.with_alpha((14 + pick * 3) as u8);
            let shine = SCALE_SHINE.with_alpha((10 + (pick % 3) * 5) as u8);
            // Scale diamond.
            let w = 6.0;
            let h = 4.5;
            ui.c.line(cx - w, cy, cx, cy - h, 1.0, SCALE_EDGE.with_alpha(55));
            ui.c.line(cx, cy - h, cx + w, cy, 1.0, SCALE_EDGE.with_alpha(55));
            ui.c.line(cx + w, cy, cx, cy + h, 1.0, glow);
            ui.c.line(cx, cy + h, cx - w, cy, 1.0, glow);
            ui.c.line(cx - w * 0.3, cy - h * 0.2, cx + w * 0.4, cy + h * 0.15, 0.5, shine);
        }
    }
}

/// Empty workspace when no `.flstheme` picture is set.
pub fn workspace(ui: &mut Ui, work: Rect, mode: usize) {
    match mode {
        1 => leather(ui, work),
        2 => snake(ui, work),
        _ => ui.c.fill_rect(work, ui.t.workspace),
    }
}

/// Mixer client backplate behind strips (Analog / Naga Swagger looks).
pub fn mixer_back(ui: &mut Ui, r: Rect, swagger: bool) {
    if swagger {
        snake(ui, r);
    } else {
        leather(ui, r);
    }
}

/// Small badge under the logo on textured workspace (optional swagger line).
pub fn swagger_caption(ui: &mut Ui, work: Rect) {
    let label = "Naga finish";
    let f = Font::condensed(11, 0);
    let w = ui.c.text_width(f, label);
    let x = work.cx() - w / 2.0;
    let y = work.cy() + 118.0;
    ui.c.text(f, x + 0.5, y + 0.5, Color::rgb(0x000000).with_alpha(120), label);
    ui.c.text(f, x, y, SCALE_SHINE.with_alpha(200), label);
}
