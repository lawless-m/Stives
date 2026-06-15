//! Water Tank — Phase 1 (docs/02-phase1-spec.md).
//!
//! A side-on tank of water on a slow see-saw tilt. Set it running and watch.
//! No game, no goal — watching is the point (docs/01-overview.md).
//!
//! The main loop is the only place the three layers meet:
//!     read input (tilt) -> step sim -> render.
//! Each layer stays ignorant of the others (docs/03-architecture.md).

mod render;
mod sim;
mod tilt;

use macroquad::prelude::*;

use sim::WaterSim;
use tilt::{TiltController, TiltMode, MANUAL_MAX_ANGLE};

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
    let mut water = WaterSim::new();
    let mut tilt = TiltController::new();
    let mut accumulator = 0.0_f32;

    loop {
        // --- Input: drive the tilt, and the UI (slider / buttons). ---
        let ui = Ui::layout();
        handle_input(&ui, &mut tilt, &mut water);

        // --- Step: fixed-timestep accumulator keeps the sim stable and the
        //     tilt and physics advancing on the same clock. ---
        accumulator += get_frame_time().min(MAX_FRAME_TIME);
        while accumulator >= sim::DT {
            tilt.advance(sim::DT);
            water.step(sim::DT, tilt.angle());
            accumulator -= sim::DT;
        }

        // --- Render: read-only draw of the sim, plus the minimal UI. ---
        clear_background(render::background_color());
        render::draw_tank(&water, tilt.angle());
        ui.draw(&tilt);

        next_frame().await;
    }
}

/// Read the mouse and update the tilt controller / sim accordingly.
fn handle_input(ui: &Ui, tilt: &mut TiltController, water: &mut WaterSim) {
    let (mx, my) = mouse_position();
    let pressed = is_mouse_button_pressed(MouseButton::Left);
    let down = is_mouse_button_down(MouseButton::Left);

    // Calm button: flatten the water.
    if pressed && ui.calm_btn.contains(mx, my) {
        water.reset();
    }

    // Auto/Manual toggle: hand control back to the see-saw.
    if pressed && ui.mode_btn.contains(mx, my) {
        match tilt.mode() {
            TiltMode::Auto => tilt.set_manual(tilt.angle()),
            TiltMode::Manual => tilt.set_auto(),
        }
    }

    // Slider: touching it takes over in manual mode (spec: slider takes over
    // when touched). Dragging keeps setting the angle absolutely.
    let grabbing = (pressed && ui.slider.contains(mx, my)) || (down && ui.dragging_zone(my));
    if grabbing {
        let t = ((mx - ui.slider.x) / ui.slider.w).clamp(0.0, 1.0);
        let angle = (t * 2.0 - 1.0) * MANUAL_MAX_ANGLE;
        tilt.set_manual(angle);
    }
}

/// A simple axis-aligned rectangle for UI hit-testing and drawing.
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
}

/// Minimal immediate-mode UI laid out along the bottom of the window. This is
/// the input/presentation glue — kept small per spec; no UI framework.
struct Ui {
    slider: Rect2,
    mode_btn: Rect2,
    calm_btn: Rect2,
}

impl Ui {
    fn layout() -> Self {
        let sw = screen_width();
        let sh = screen_height();
        let bar_y = sh - 56.0;
        let btn_w = 110.0;
        let btn_h = 32.0;
        let pad = 16.0;

        let calm_btn = Rect2 {
            x: sw - pad - btn_w,
            y: bar_y,
            w: btn_w,
            h: btn_h,
        };
        let mode_btn = Rect2 {
            x: calm_btn.x - pad - btn_w,
            y: bar_y,
            w: btn_w,
            h: btn_h,
        };
        let slider = Rect2 {
            x: pad,
            y: bar_y + btn_h * 0.5 - 4.0,
            w: mode_btn.x - 2.0 * pad,
            h: 8.0,
        };

        Ui {
            slider,
            mode_btn,
            calm_btn,
        }
    }

    /// Generous vertical band around the slider so a drag keeps tracking even
    /// if the cursor strays off the thin track.
    fn dragging_zone(&self, my: f32) -> bool {
        (my - (self.slider.y + self.slider.h * 0.5)).abs() < 28.0
    }

    fn draw(&self, tilt: &TiltController) {
        // Slider track.
        let track = self.slider;
        draw_rectangle(track.x, track.y, track.w, track.h, Color::new(0.25, 0.27, 0.32, 1.0));
        // Centre tick (zero tilt).
        let mid_x = track.x + track.w * 0.5;
        draw_line(mid_x, track.y - 4.0, mid_x, track.y + track.h + 4.0, 1.0, GRAY);

        // Slider handle, positioned from the current angle.
        let t = (tilt.angle() / MANUAL_MAX_ANGLE).clamp(-1.0, 1.0) * 0.5 + 0.5;
        let hx = track.x + t * track.w;
        let hy = track.y + track.h * 0.5;
        let handle_color = match tilt.mode() {
            TiltMode::Manual => Color::new(0.55, 0.80, 1.00, 1.0),
            TiltMode::Auto => Color::new(0.55, 0.58, 0.65, 1.0),
        };
        draw_circle(hx, hy, 11.0, handle_color);

        // Buttons.
        let mode_label = match tilt.mode() {
            TiltMode::Auto => "Mode: Auto",
            TiltMode::Manual => "Mode: Manual",
        };
        draw_button(self.mode_btn, mode_label);
        draw_button(self.calm_btn, "Calm");

        // Hint.
        draw_text(
            "Drag the slider to tip the tank.  Mode toggles the see-saw.",
            track.x,
            track.y - 16.0,
            18.0,
            Color::new(0.7, 0.73, 0.78, 1.0),
        );
    }
}

fn draw_button(r: Rect2, label: &str) {
    let (mx, my) = mouse_position();
    let hot = r.contains(mx, my);
    let bg = if hot {
        Color::new(0.28, 0.31, 0.38, 1.0)
    } else {
        Color::new(0.20, 0.22, 0.27, 1.0)
    };
    draw_rectangle(r.x, r.y, r.w, r.h, bg);
    draw_rectangle_lines(r.x, r.y, r.w, r.h, 2.0, Color::new(0.45, 0.48, 0.55, 1.0));
    let size = 18.0;
    let dim = measure_text(label, None, size as u16, 1.0);
    draw_text(
        label,
        r.x + (r.w - dim.width) * 0.5,
        r.y + (r.h + dim.height) * 0.5,
        size,
        WHITE,
    );
}
