// ===== modes/matrix.rs =====

use crate::ansi::{DIM, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::{RngExt};

struct Drop {
    x: i32,
    y: f64,
    speed: f64,
    length: i32,
    seed: i32,
}

pub struct MatrixMode {
    speed_factor: f64,
    color_provider: ColorProvider,
    drops: Vec<Drop>,
    last_width: u16,
}

impl MatrixMode {
    pub fn new(speed_factor: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed_factor,
            color_provider,
            drops: Vec::new(),
            last_width: 0,
        }
    }

    /// Initializes or resizes the rain columns based on terminal width.
    fn populate_drops(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        self.drops.clear();
        
        // Create 1-2 drops per column for a dense look
        for x in 0..width {
            let num_drops = rng.random_range(1..=2);
            for _ in 0..num_drops {
                self.drops.push(Drop {
                    x: x as i32,
                    // Stagger the starting Y positions above the screen
                    y: rng.random_range(-(height as f64 * 2.0)..0.0),
                    speed: rng.random_range(10.0..25.0) * self.speed_factor,
                    length: rng.random_range(10..25),
                    seed: rng.random_range(0..10000),
                });
            }
        }
        self.last_width = width;
    }
}

impl Mode for MatrixMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let mut rng = rand::rng();

        // Re-populate if the terminal is resized
        if width != self.last_width || self.drops.is_empty() {
            self.populate_drops(width, height);
        }

        for drop in &mut self.drops {
            drop.y += drop.speed * dt;

            // Reset the drop to the top once its tail fully clears the bottom
            if drop.y - (drop.length as f64) > height as f64 {
                drop.y = rng.random_range(-10.0..-2.0);
                drop.speed = rng.random_range(10.0..25.0) * self.speed_factor;
                drop.length = rng.random_range(10..25);
            }
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
        let mut buf: Vec<Vec<String>> = vec![vec![" ".to_string(); w]; h];
        let mut rng = rand::rng();

        // A mix of numbers, letters, and symbols for the classic look
        let matrix_chars: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ@#$%&*";

        for drop in &self.drops {
            let head_y = drop.y as i32;
            let base_col = self.color_provider.get(t_abs, drop.seed);

            for i in 0..drop.length {
                let y = head_y - i;
                let x = drop.x;

                if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                    let ch_idx = rng.random_range(0..matrix_chars.len());
                    let ch = matrix_chars[ch_idx] as char;

                    let (col, display_ch) = if i == 0 {
                        // The leading character is bright white
                        ("\x1b[97m".to_string(), ch.to_string()) 
                    } else if i < drop.length / 2 {
                        // The top half of the tail is the base color
                        (base_col.clone(), ch.to_string())
                    } else {
                        // The bottom half dims out
                        (format!("{}{}", DIM, base_col), ch.to_string())
                    };

                    buf[y as usize][x as usize] = format!("{}{}{}", col, display_ch, RESET);
                }
            }
        }

        buf.iter()
            .map(|row| row.join(""))
            .collect::<Vec<_>>()
            .join("\n")
    }
}