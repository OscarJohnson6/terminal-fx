// ===== src/color.rs =====

use crate::ansi::rgb;

pub type Rgb = (u8, u8, u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Rainbow,
    Ocean,
    Sunset,
    Matrix,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorProvider {
    pub mode: ColorMode,
    pub speed: f64,
}

impl ColorProvider {
    pub fn new(mode: ColorMode, speed: f64) -> Self {
        Self { mode, speed }
    }

    pub fn get(&self, t: f64, x: i32) -> String {
        match self.mode {
            ColorMode::Rainbow => {
                let r = ((t * self.speed + x as f64 * 0.1).sin() * 127.0 + 128.0) as u8;
                let g = ((t * self.speed + x as f64 * 0.1 + 2.0).sin() * 127.0 + 128.0) as u8;
                let b = ((t * self.speed + x as f64 * 0.1 + 4.0).sin() * 127.0 + 128.0) as u8;
                rgb(r, g, b)
            }
            ColorMode::Ocean => self.lerp_color(t, x, (0, 20, 100), (64, 224, 208)),
            ColorMode::Sunset => self.lerp_color(t, x, (100, 60, 40), (255, 120, 80)),
            ColorMode::Matrix => rgb(0, 255, 70),
        }
    }

    fn lerp_color(&self, t: f64, x: i32, low: Rgb, high: Rgb) -> String {
        let factor = ((t * self.speed + x as f64 * 0.05).sin() + 1.0) / 2.0;

        let r = (low.0 as f64 + (high.0 as f64 - low.0 as f64) * factor) as u8;
        let g = (low.1 as f64 + (high.1 as f64 - low.1 as f64) * factor) as u8;
        let b = (low.2 as f64 + (high.2 as f64 - low.2 as f64) * factor) as u8;

        rgb(r, g, b)
    }

    pub fn tint(&self, base: Rgb, t: f64, x: i32, y: i32) -> Rgb {
        let (r, g, b) = base;

        match self.mode {
            ColorMode::Rainbow => {
                // Fluctuates colors slightly based on a sine wave moving across the screen.
                let shift = ((t * self.speed * 2.0 + (x + y) as f64 * 0.1).sin() * 30.0) as i32;

                let r_new = (r as i32 + shift).clamp(0, 255) as u8;
                let g_new = (g as i32 - shift / 2).clamp(0, 255) as u8;
                let b_new = (b as i32 + shift / 3).clamp(0, 255) as u8;

                (r_new, g_new, b_new)
            }
            ColorMode::Ocean => {
                // Crush reds, boost blues and greens slightly.
                let r_new = (r as f64 * 0.4) as u8;
                let g_new = (g as f64 * 0.9).min(255.0) as u8;
                let b_new = (b as f64 * 1.3).min(255.0) as u8;

                (r_new, g_new, b_new)
            }
            ColorMode::Sunset => {
                // Boost reds/oranges, crush blues.
                let r_new = (r as f64 * 1.3).min(255.0) as u8;
                let g_new = (g as f64 * 0.8).min(255.0) as u8;
                let b_new = (b as f64 * 0.5) as u8;

                (r_new, g_new, b_new)
            }
            ColorMode::Matrix => {
                // Calculate grayscale brightness/luma, then tint Matrix green.
                let luma = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
                (0, (luma as f64 * 1.2).min(255.0) as u8, 0)
            }
        }
    }
}
