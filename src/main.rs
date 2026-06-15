//! Water Tank (docs/).
//!
//! A tank of water on a slow see-saw tilt. Set it running and watch — no game,
//! no goal (docs/01-overview.md).
//!
//! Two modes, chosen by a command-line argument:
//!   * default        — Phase 1 side-on 1D tank (docs/02-phase1-spec.md)
//!   * `2d` / `iso`    — Phase 2 isometric 2D tank (docs/04-phase2-isometric.md)
//!
//! Each mode wires three ignorant layers together in its run loop:
//!     read input (tilt) -> step sim -> render.
//! The sim never knows how it is drawn (docs/03-architecture.md), which is what
//! let Phase 2 be added alongside Phase 1 rather than as a rewrite.

mod render;
mod render_iso;
mod sim;
mod sim2d;
mod tilt;

use macroquad::prelude::*;

use sim::WaterSim;
use sim2d::WaterSim2d;
use tilt::{
    Tilt2dController, TiltController, TiltMode, MANUAL_MAX_ANGLE, TILT_2D_MAX_ANGLE,
};

/// Clamp on real frame time fed to the accumulator, to avoid a "spiral of
/// death" if the window stalls (e.g. while being dragged).
const MAX_FRAME_TIME: f32 = 0.05;

fn window_conf() -> Conf {
    Conf {
        window_title: "Water Tank".to_owned(),
        window_width: 1000,
        window_height: 600,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let iso = std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "2d" | "--2d" | "iso" | "--iso"));

    if iso {
        run_iso().await;
    } else {
        run_side_on().await;
    }
}

// ----------------------------------------------------------------------------
// Phase 1 — side-on 1D tank.
// ----------------------------------------------------------------------------

async fn run_side_on() {
    let mut water = WaterSim::new();
    let mut tilt = TiltController::new();
    let mut accumulator = 0.0_f32;

    loop {
        let ui = Ui::layout();
        handle_input_side_on(&ui, &mut tilt, &mut water);

        accumulator += get_frame_time().min(MAX_FRAME_TIME);
        while accumulator >= sim::DT {
            tilt.advance(sim::DT);
            water.step(sim::DT, tilt.angle());
            accumulator -= sim::DT;
        }

        clear_background(render::background_color());
        render::draw_tank(&water, tilt.angle());
        ui.draw(&tilt);

        next_frame().await;
    }
}

fn handle_input_side_on(ui: &Ui, tilt: &mut TiltController, water: &mut WaterSim) {
    let (mx, my) = mouse_position();
    let pressed = is_mouse_button_pressed(MouseButton::Left);
    let down = is_mouse_button_down(MouseButton::Left);

    if pressed && ui.calm_btn.contains(mx, my) {
        water.reset();
    }
    if pressed && ui.mode_btn.contains(mx, my) {
        match tilt.mode() {
            TiltMode::Auto => tilt.set_manual(tilt.angle()),
            TiltMode::Manual => tilt.set_auto(),
        }
    }

    // Slider: touching it takes over in manual mode; dragging keeps setting.
    let grabbing = (pressed && ui.slider.contains(mx, my)) || (down && ui.dragging_zone(my));
    if grabbing {
        let t = ((mx - ui.slider.x) / ui.slider.w).clamp(0.0, 1.0);
        tilt.set_manual((t * 2.0 - 1.0) * MANUAL_MAX_ANGLE);
    }
}

// ----------------------------------------------------------------------------
// Phase 2 — isometric 2D tank.
// ----------------------------------------------------------------------------

async fn run_iso() {
    let mut water = WaterSim2d::new();
    let mut tilt = Tilt2dController::new();
    let mut accumulator = 0.0_f32;

    loop {
        let ui = Ui2d::layout();
        handle_input_iso(&ui, &mut tilt, &mut water);

        accumulator += get_frame_time().min(MAX_FRAME_TIME);
        while accumulator >= sim::DT {
            tilt.advance(sim::DT);
            water.step(sim::DT, tilt.tilt_x(), tilt.tilt_y());
            accumulator -= sim::DT;
        }

        clear_background(render_iso::background_color());
        render_iso::draw_tank(&water);
        ui.draw(&tilt);

        next_frame().await;
    }
}

fn handle_input_iso(ui: &Ui2d, tilt: &mut Tilt2dController, water: &mut WaterSim2d) {
    let (mx, my) = mouse_position();
    let pressed = is_mouse_button_pressed(MouseButton::Left);
    let down = is_mouse_button_down(MouseButton::Left);

    if pressed && ui.calm_btn.contains(mx, my) {
        water.reset();
    }
    if pressed && ui.mode_btn.contains(mx, my) {
        match tilt.mode() {
            TiltMode::Auto => tilt.set_manual(tilt.tilt_x(), tilt.tilt_y()),
            TiltMode::Manual => tilt.set_auto(),
        }
    }

    // Tilt pad: touch/drag inside to set the 2D tilt absolutely.
    let pad = ui.pad;
    let grabbing = (pressed && pad.contains(mx, my)) || (down && ui.dragging && pad.near(mx, my));
    if grabbing {
        let nx = ((mx - (pad.x + pad.w * 0.5)) / (pad.w * 0.5)).clamp(-1.0, 1.0);
        // Screen y is down; up on the pad means +y tilt.
        let ny = (((pad.y + pad.h * 0.5) - my) / (pad.h * 0.5)).clamp(-1.0, 1.0);
        tilt.set_manual(nx * TILT_2D_MAX_ANGLE, ny * TILT_2D_MAX_ANGLE);
    }
    ui_set_dragging(grabbing);
}

// A tiny bit of drag-tracking state for the pad, kept in a thread-local so the
// run loop stays free of extra plumbing.
thread_local! {
    static DRAGGING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
fn ui_set_dragging(v: bool) {
    DRAGGING.with(|d| d.set(v));
}
fn ui_dragging() -> bool {
    DRAGGING.with(|d| d.get())
}

// ----------------------------------------------------------------------------
// Minimal immediate-mode UI helpers (input/presentation glue).
// ----------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Rect2 {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Rect2 {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.w && py >= self.y && py <= self.y + self.h
    }
    /// Generous hit-test so a drag keeps tracking just outside the rect.
    fn near(&self, px: f32, py: f32) -> bool {
        let m = 40.0;
        px >= self.x - m && px <= self.x + self.w + m && py >= self.y - m && py <= self.y + self.h + m
    }
}

/// Side-on UI: slider + mode toggle + calm, along the bottom.
struct Ui {
    slider: Rect2,
    mode_btn: Rect2,
    calm_btn: Rect2,
}

impl Ui {
    fn layout() -> Self {
        let (sw, sh) = (screen_width(), screen_height());
        let bar_y = sh - 56.0;
        let (btn_w, btn_h, pad) = (110.0, 32.0, 16.0);

        let calm_btn = Rect2 { x: sw - pad - btn_w, y: bar_y, w: btn_w, h: btn_h };
        let mode_btn = Rect2 { x: calm_btn.x - pad - btn_w, y: bar_y, w: btn_w, h: btn_h };
        let slider = Rect2 {
            x: pad,
            y: bar_y + btn_h * 0.5 - 4.0,
            w: mode_btn.x - 2.0 * pad,
            h: 8.0,
        };
        Ui { slider, mode_btn, calm_btn }
    }

    fn dragging_zone(&self, my: f32) -> bool {
        (my - (self.slider.y + self.slider.h * 0.5)).abs() < 28.0
    }

    fn draw(&self, tilt: &TiltController) {
        let track = self.slider;
        draw_rectangle(track.x, track.y, track.w, track.h, Color::new(0.25, 0.27, 0.32, 1.0));
        let mid_x = track.x + track.w * 0.5;
        draw_line(mid_x, track.y - 4.0, mid_x, track.y + track.h + 4.0, 1.0, GRAY);

        let t = (tilt.angle() / MANUAL_MAX_ANGLE).clamp(-1.0, 1.0) * 0.5 + 0.5;
        let hx = track.x + t * track.w;
        let hy = track.y + track.h * 0.5;
        draw_circle(hx, hy, 11.0, handle_color(tilt.mode()));

        let mode_label = match tilt.mode() {
            TiltMode::Auto => "Mode: Auto",
            TiltMode::Manual => "Mode: Manual",
        };
        draw_button(self.mode_btn, mode_label);
        draw_button(self.calm_btn, "Calm");
        draw_hint("Drag the slider to tip the tank.  Mode toggles the see-saw.", track.x, track.y - 16.0);
    }
}

/// Iso UI: a 2D tilt pad + mode toggle + calm.
struct Ui2d {
    pad: Rect2,
    mode_btn: Rect2,
    calm_btn: Rect2,
    dragging: bool,
}

impl Ui2d {
    fn layout() -> Self {
        let (sw, sh) = (screen_width(), screen_height());
        let bar_y = sh - 56.0;
        let (btn_w, btn_h, pad_gap) = (110.0, 32.0, 16.0);

        let calm_btn = Rect2 { x: sw - pad_gap - btn_w, y: bar_y, w: btn_w, h: btn_h };
        let mode_btn = Rect2 { x: calm_btn.x - pad_gap - btn_w, y: bar_y, w: btn_w, h: btn_h };
        let pad_size = 88.0;
        let pad = Rect2 { x: pad_gap, y: sh - pad_gap - pad_size, w: pad_size, h: pad_size };
        Ui2d { pad, mode_btn, calm_btn, dragging: ui_dragging() }
    }

    fn draw(&self, tilt: &Tilt2dController) {
        let p = self.pad;
        draw_rectangle(p.x, p.y, p.w, p.h, Color::new(0.16, 0.18, 0.22, 1.0));
        draw_rectangle_lines(p.x, p.y, p.w, p.h, 2.0, Color::new(0.40, 0.43, 0.50, 1.0));
        let (pcx, pcy) = (p.x + p.w * 0.5, p.y + p.h * 0.5);
        draw_line(p.x, pcy, p.x + p.w, pcy, 1.0, Color::new(0.30, 0.33, 0.40, 1.0));
        draw_line(pcx, p.y, pcx, p.y + p.h, 1.0, Color::new(0.30, 0.33, 0.40, 1.0));

        // Current tilt as a dot in the pad.
        let dx = (tilt.tilt_x() / TILT_2D_MAX_ANGLE).clamp(-1.0, 1.0) * (p.w * 0.5);
        let dy = (tilt.tilt_y() / TILT_2D_MAX_ANGLE).clamp(-1.0, 1.0) * (p.h * 0.5);
        draw_circle(pcx + dx, pcy - dy, 7.0, handle_color(tilt.mode()));

        let mode_label = match tilt.mode() {
            TiltMode::Auto => "Mode: Auto",
            TiltMode::Manual => "Mode: Manual",
        };
        draw_button(self.mode_btn, mode_label);
        draw_button(self.calm_btn, "Calm");
        draw_hint("Drag in the pad to tip the tank in 2D.  Auto precesses.", p.x, p.y - 12.0);
    }
}

fn handle_color(mode: TiltMode) -> Color {
    match mode {
        TiltMode::Manual => Color::new(0.55, 0.80, 1.00, 1.0),
        TiltMode::Auto => Color::new(0.55, 0.58, 0.65, 1.0),
    }
}

fn draw_hint(text: &str, x: f32, y: f32) {
    draw_text(text, x, y, 18.0, Color::new(0.7, 0.73, 0.78, 1.0));
}

fn draw_button(r: Rect2, label: &str) {
    let (mx, my) = mouse_position();
    let bg = if r.contains(mx, my) {
        Color::new(0.28, 0.31, 0.38, 1.0)
    } else {
        Color::new(0.20, 0.22, 0.27, 1.0)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.45, 0.48, 0.55, 1.0));
    let size = 18.0;
    let dim = measure_text(label, None, size as u16, 1.0);
    draw_text(label, r.x + (r.w - dim.width) * 0.5, r.y + (r.h + dim.height) * 0.5, size, WHITE);
}
