// ===== src/modes/plasma.rs =====
//
// Classic demo-scene plasma effect. Each pixel's colour is determined by
// summing several sine waves in x, y, and time. The result cycles smoothly
// through the full colour spectrum giving a liquid, pulsing look.
//
// We use the half-block trick (▀) for 2× vertical resolution.

use crate::ansi::RESET;
use crate::color::ColorProvider;
use crate::mode_base::Mode;

pub struct PlasmaMode {
    speed: f64,
    _color: ColorProvider,
}

impl PlasmaMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self { speed, _color: color }
    }
}

/// Map a value in 0..1 to a vivid Rgb using a hue rotation.
/// We use three out-of-phase cosines (the "LED rainbow" formula).
fn hue_to_rgb(h: f64) -> (u8, u8, u8) {
    let r = (0.5 + 0.5 * (h * std::f64::consts::TAU                ).cos() * 255.0) as u8;
    let g = (0.5 + 0.5 * (h * std::f64::consts::TAU + 2.094        ).cos() * 255.0) as u8;
    let b = (0.5 + 0.5 * (h * std::f64::consts::TAU + 4.189        ).cos() * 255.0) as u8;
    (r, g, b)
}

impl Mode for PlasmaMode {
    fn update(&mut self, _dt: f64, _w: u16, _h: u16, _t: f64) {}

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w  = width  as usize;
        let h  = height as usize;
        let ph = h * 2;
        let t  = t_abs * self.speed;

        let mut out = String::with_capacity(w * h * 42);

        for row in 0..h {
            for col in 0..w {
                // Compute colour for upper and lower pixel of this cell
                let upper = plasma_val(col, row * 2,     w, ph, t);
                let lower = plasma_val(col, row * 2 + 1, w, ph, t);
                let (ur,ug,ub) = hue_to_rgb(upper);
                let (lr,lg,lb) = hue_to_rgb(lower);
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur,ug,ub, lr,lg,lb,
                ));
            }
            out.push_str(RESET);
            if row < h-1 { out.push('\n'); }
        }
        out
    }
}

/// Returns a value in roughly 0..1 for position (px, py) at time t.
fn plasma_val(px: usize, py: usize, w: usize, h: usize, t: f64) -> f64 {
    let x = px as f64 / w as f64;
    let y = py as f64 / h as f64;
    let cx = x - 0.5;
    let cy = y - 0.5;

    let v = (x * 12.0 + t * 1.1).sin()
          + (y * 10.0 + t * 0.9).sin()
          + ((x * 8.0 + t * 0.7).sin() + (y * 9.0 + t * 1.3).sin()) * 0.5
          + ((cx * cx + cy * cy).sqrt() * 18.0 - t * 1.4).sin()
          + (x * 6.0 + y * 8.0 + t * 1.2).sin() * 0.6
          + ((cx + 0.3 * (t * 0.5).sin()).powi(2)
             + (cy + 0.3 * (t * 0.7).cos()).powi(2)).sqrt() * 14.0;

    // v is roughly -4..4 ; normalise to 0..1
    (v / 8.0 + 0.5).rem_euclid(1.0)
}
