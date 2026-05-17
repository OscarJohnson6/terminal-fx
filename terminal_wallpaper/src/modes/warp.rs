// ===== src/modes/warp.rs =====
//
// Hyperspace warp drive. Stars exist in 3-D space (x, y, z). Each frame z
// decreases (stars rush toward the camera). When a star is close enough it
// leaves a motion-blur streak from its previous screen position to its
// current one.

use crate::ansi::RESET;
use crate::color::{ColorProvider, ColorMode};
use crate::mode_base::Mode;
use rand::RngExt;

struct Star3D {
    x: f64, y: f64, z: f64,
    ox: f64, oy: f64,
    has_prev: bool,
}

const MAX_Z: f64 = 1.0;
const MIN_Z: f64 = 0.001;

pub struct WarpMode {
    speed: f64,
    color: ColorProvider,
    stars: Vec<Star3D>,
}

impl WarpMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        let mut rng = rand::rng();
        let stars = (0..320).map(|_| Star3D {
            x:  rng.random_range(-0.5..0.5),
            y:  rng.random_range(-0.5..0.5),
            z:  rng.random_range(MIN_Z..MAX_Z),
            ox: 0.0, oy: 0.0, has_prev: false,
        }).collect();
        Self { speed, color, stars }
    }

    fn project(s: &Star3D, w: usize, h: usize) -> (f64, f64) {
        let scale = (w as f64 * 0.9).min(h as f64 * 1.8);
        let sx    = s.x / s.z * scale + w as f64 * 0.5;
        let sy    = s.y / s.z * scale + h as f64 * 0.5;
        (sx, sy)
    }
}

impl Mode for WarpMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t: f64) {
        let mut rng = rand::rng();
        let w = width as usize;
        let h = height as usize;
        let warp = dt * self.speed * 0.55;

        for s in &mut self.stars {
            let (sx, sy) = Self::project(s, w, h);
            s.ox = sx; s.oy = sy; s.has_prev = true;
            s.z -= s.z * warp * 2.2;

            if s.z < MIN_Z || sx < 0.0 || sx >= w as f64 || sy < 0.0 || sy >= h as f64 {
                s.x = rng.random_range(-0.5..0.5);
                s.y = rng.random_range(-0.5..0.5);
                s.z = rng.random_range(MAX_Z * 0.7..MAX_Z);
                s.has_prev = false;
            }
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w  = width  as usize;
        let h  = height as usize;
        let ph = h * 2;

        // Subtle center glow pulse — uses t_abs
        let pulse = (t_abs * 1.8).sin() * 0.5 + 0.5; // 0..1
        let cx = w as f64 * 0.5;
        let cy = ph as f64 * 0.5;

        let mut pix = vec![vec![(0u8, 0u8, 8u8); w]; ph];

        // Draw center glow in background
        for py in 0..ph {
            for px in 0..w {
                let dx = px as f64 - cx;
                let dy = (py as f64 - cy) * 0.5;
                let d  = (dx * dx + dy * dy).sqrt();
                let glow = (1.0 - (d / (w as f64 * 0.6)).min(1.0)).powi(3) * pulse;
                let base = (glow * 35.0) as u8;
                if base > 0 {
                    pix[py][px] = (base / 3, base / 2, base + 20);
                }
            }
        }

        for s in &self.stars {
            let (sx, sy) = Self::project(s, w, h);
            let depth    = (s.z / MAX_Z).clamp(0.0, 1.0);
            let bright   = (1.0 - depth).powi(2);

            let col: (u8, u8, u8) = match self.color.mode {
                ColorMode::Matrix => {
                    let v = (bright * 230.0) as u8;
                    (0, v, (v as f64 * 0.3) as u8)
                }
                ColorMode::Ocean => {
                    let b = (bright * 255.0) as u8;
                    let g = (bright * 200.0) as u8;
                    (0, g, b)
                }
                ColorMode::Sunset => {
                    let r = (bright * 255.0) as u8;
                    let g = (bright * 150.0) as u8;
                    (r, g, 20)
                }
                _ => {
                    if depth > 0.5 {
                        let t = (depth - 0.5) / 0.5;
                        let v = ((1.0 - t) * 255.0) as u8;
                        (0, (v as f64 * 0.4) as u8, v)
                    } else if depth > 0.1 {
                        let t = (depth - 0.1) / 0.4;
                        let v = 255u8;
                        let b = (t * 255.0) as u8;
                        (v, v, b)
                    } else {
                        (255, (depth / 0.1 * 100.0) as u8, 0)
                    }
                }
            };

            let (ox, oy) = if s.has_prev { (s.ox, s.oy) } else { (sx, sy) };
            let steps    = ((sx - ox).abs().max((sy - oy).abs()) as usize + 1).max(1).min(24);
            for i in 0..=steps {
                let t  = i as f64 / steps as f64;
                let px = (ox + (sx - ox) * t).round() as i32;
                let py = ((oy + (sy - oy) * t) * 2.0).round() as i32;
                if px >= 0 && px < w as i32 && py >= 0 && py < ph as i32 {
                    let tail_fade = 0.2 + 0.8 * t;
                    let (r, g, b) = col;
                    pix[py as usize][px as usize] = (
                        (r as f64 * tail_fade) as u8,
                        (g as f64 * tail_fade) as u8,
                        (b as f64 * tail_fade) as u8,
                    );
                }
            }
        }

        let mut out = String::with_capacity(w * h * 42);
        for row in 0..h {
            for col in 0..w {
                let (ur, ug, ub) = pix[row * 2][col];
                let (lr, lg, lb) = if row * 2 + 1 < ph { pix[row * 2 + 1][col] } else { (0, 0, 0) };
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur, ug, ub, lr, lg, lb
                ));
            }
            out.push_str(RESET);
            if row < h - 1 { out.push('\n'); }
        }
        out
    }
}