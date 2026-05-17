// ===== src/modes/metaballs.rs =====
//
// Lava-lamp metaballs. Each ball contributes field strength = r²/d² to every
// pixel. Above THRESHOLD, pixel is inside a blob. Specular highlight from the
// field gradient gives a 3D bubbled look. Hue slowly rotates with t_abs.

use crate::ansi::RESET;
use crate::color::{ColorProvider, ColorMode};
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

struct Ball {
    x: f64, y: f64,
    vx: f64, vy: f64,
    r: f64,
    hue: f64,
}

pub struct MetaballsMode {
    speed:       f64,
    color:       ColorProvider,
    balls:       Vec<Ball>,
    initialized: bool,
}

const THRESHOLD:   f64 = 0.85;
const GLOW_THRESH: f64 = 0.45;

impl MetaballsMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self { speed, color, balls: Vec::new(), initialized: false }
    }

    fn init(&mut self, w: usize, ph: usize) {
        let mut rng = rand::rng();
        let n = rng.random_range(4usize..=7);
        self.balls = (0..n).map(|i| {
            let angle = rng.random_range(0.0..TAU);
            let spd   = rng.random_range(4.0..12.0) * self.speed;
            Ball {
                x:  rng.random_range(w as f64 * 0.1..w as f64 * 0.9),
                y:  rng.random_range(ph as f64 * 0.1..ph as f64 * 0.9),
                vx: angle.cos() * spd,
                vy: angle.sin() * spd * 0.5,
                r:  rng.random_range(5.0..18.0),
                hue: i as f64 / n as f64,
            }
        }).collect();
        self.initialized = true;
    }
}

fn hue_rgb(hue: f64, mode: ColorMode) -> (u8, u8, u8) {
    let h6 = (hue * 6.0).rem_euclid(6.0);
    let (r, g, b) = if      h6 < 1.0 { (1.0,        h6,         0.0) }
                    else if h6 < 2.0 { (2.0 - h6,   1.0,        0.0) }
                    else if h6 < 3.0 { (0.0,        1.0,        h6 - 2.0) }
                    else if h6 < 4.0 { (0.0,        4.0 - h6,   1.0) }
                    else if h6 < 5.0 { (h6 - 4.0,   0.0,        1.0) }
                    else             { (1.0,        0.0,        6.0 - h6) };
    let (r, g, b) = ((r * 240.0) as u8, (g * 240.0) as u8, (b * 240.0) as u8);
    match mode {
        ColorMode::Matrix => (0, ((r as u32 + g as u32 + b as u32) / 3) as u8 + 40, 0),
        ColorMode::Ocean  => (0, (g / 2).saturating_add(30), b),
        ColorMode::Sunset => (r, (g as f64 * 0.6) as u8, (b as f64 * 0.3) as u8),
        _                 => (r, g, b),
    }
}

impl Mode for MetaballsMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t: f64) {
        let ph = height as usize * 2;
        let w  = width  as usize;
        if !self.initialized { self.init(w, ph); return; }
        let dt = dt * self.speed;

        for b in &mut self.balls {
            b.x += b.vx * dt;
            b.y += b.vy * dt;
            if b.x < b.r       || b.x > w  as f64 - b.r { b.vx = -b.vx; b.x = b.x.clamp(b.r,       w  as f64 - b.r); }
            if b.y < b.r * 0.5 || b.y > ph as f64 - b.r { b.vy = -b.vy; b.y = b.y.clamp(b.r * 0.5, ph as f64 - b.r); }
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w  = width  as usize;
        let h  = height as usize;
        let ph = h * 2;

        // Slowly rotating hue offset — makes the whole scene shift color over time
        let hue_shift = t_abs * 0.05;

        let mut pix = vec![vec![(4u8, 4u8, 12u8); w]; ph];

        for py in 0..ph {
            for px in 0..w {
                let mut field       = 0.0_f64;
                let mut dom_hue     = 0.0_f64;
                let mut dom_weight  = 0.0_f64;
                let mut gx          = 0.0_f64;
                let mut gy          = 0.0_f64;

                for b in &self.balls {
                    let dx = px as f64 - b.x;
                    let dy = (py as f64 - b.y) * 2.0;
                    let d2 = dx * dx + dy * dy;
                    if d2 < 1e-6 { continue; }
                    let contrib = b.r * b.r / d2;
                    field += contrib;
                    if contrib > dom_weight { dom_weight = contrib; dom_hue = b.hue; }
                    gx += -2.0 * b.r * b.r * dx / (d2 * d2);
                    gy += -2.0 * b.r * b.r * dy / (d2 * d2);
                }

                if field >= THRESHOLD {
                    let base = hue_rgb(dom_hue + hue_shift, self.color.mode);
                    let gn   = (gx * gx + gy * gy).sqrt().max(1e-9);
                    let (nx, ny) = (gx / gn, gy / gn);
                    let (lx, ly) = (-0.707, -0.707);
                    let spec     = (nx * lx + ny * ly).clamp(0.0, 1.0).powi(8);
                    let diff     = (nx * lx + ny * ly).clamp(0.0, 1.0) * 0.5 + 0.5;
                    let r = ((base.0 as f64 * diff + spec * 180.0).min(255.0)) as u8;
                    let g = ((base.1 as f64 * diff + spec * 180.0).min(255.0)) as u8;
                    let b = ((base.2 as f64 * diff + spec * 180.0).min(255.0)) as u8;
                    pix[py][px] = (r, g, b);

                } else if field >= GLOW_THRESH {
                    let t    = ((field - GLOW_THRESH) / (THRESHOLD - GLOW_THRESH)).powi(2);
                    let base = hue_rgb(dom_hue + hue_shift, self.color.mode);
                    let glow = ((base.0 as f64 * 0.4) as u8,
                                (base.1 as f64 * 0.4) as u8,
                                (base.2 as f64 * 0.4) as u8);
                    let bg = pix[py][px];
                    pix[py][px] = (
                        (bg.0 as f64 + (glow.0 as f64 - bg.0 as f64) * t) as u8,
                        (bg.1 as f64 + (glow.1 as f64 - bg.1 as f64) * t) as u8,
                        (bg.2 as f64 + (glow.2 as f64 - bg.2 as f64) * t) as u8,
                    );
                }
            }
        }

        let mut out = String::with_capacity(w * h * 44);
        for row in 0..h {
            for col in 0..w {
                let (ur, ug, ub) = pix[row * 2][col];
                let (lr, lg, lb) = if row * 2 + 1 < ph { pix[row * 2 + 1][col] } else { (0, 0, 0) };
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur, ug, ub, lr, lg, lb,
                ));
            }
            out.push_str(RESET);
            if row < h - 1 { out.push('\n'); }
        }
        out
    }
}