// ===== modes/fish_tank.rs =====

use crate::ansi::{DIM, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::{RngExt};

struct Fish {
    x: f64,
    y: f64,
    speed: f64,
    direction: f64, // 1.0 for right, -1.0 for left
    body: String,
    seed: i32,
}

struct Bubble {
    x: f64,
    y: f64,
    speed: f64,
}

pub struct FishTankMode {
    color_provider: ColorProvider,
    fish: Vec<Fish>,
    bubbles: Vec<Bubble>,
    seaweed_seeds: Vec<(usize, i32)>, // (x_position, height)
    initialized: bool,
}

impl FishTankMode {
    pub fn new(_speed_factor: f64, color_provider: ColorProvider) -> Self {
        Self {
            color_provider,
            fish: Vec::new(),
            bubbles: Vec::new(),
            seaweed_seeds: Vec::new(),
            initialized: false,
        }
    }

    fn init(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        
        // Spawn a variety of fish
        let fish_types = vec!["<*))>>{", "><>", "><(((('>", "_/<((((º>", " >^)))><"];
        for _ in 0..8 {
            self.fish.push(Fish {
                x: rng.random_range(0.0..width as f64),
                y: rng.random_range(2.0..height as f64 - 2.0),
                speed: rng.random_range(3.0..8.0),
                direction: if rng.random_bool(0.5) { 1.0 } else { -1.0 },
                body: fish_types[rng.random_range(0..fish_types.len())].to_string(),
                seed: rng.random_range(0..1000),
            });
        }

        // Generate seaweed along the bottom
        for x in (2..width as usize - 2).step_by(5) {
            if rng.random_bool(0.7) {
                self.seaweed_seeds.push((x, rng.random_range(3..height as i32 / 3)));
            }
        }
        self.initialized = true;
    }
}

impl Mode for FishTankMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if !self.initialized { self.init(width, height); }
        let mut rng = rand::rng();

        // Update Fish
        for fish in &mut self.fish {
            fish.x += fish.speed * fish.direction * dt;
            // Wrap around screen
            if fish.x > width as f64 + 10.0 { fish.x = -10.0; }
            if fish.x < -10.0 { fish.x = width as f64 + 10.0; }
            
            // Randomly bob up and down
            fish.y += (rng.random_range(-1.0..1.0) * dt).clamp(-1.0, 1.0);
        }

        // Update Bubbles
        self.bubbles.retain_mut(|b| {
            b.y -= b.speed * dt;
            b.x += rng.random_range(-2.0..2.0) * dt; // Slight wobble
            b.y > 0.0
        });

        if rng.random_bool(0.1) {
            self.bubbles.push(Bubble {
                x: rng.random_range(0.0..width as f64),
                y: height as f64,
                speed: rng.random_range(5.0..10.0),
            });
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
        let mut buf = vec![vec![" ".to_string(); w]; h];

        // 1. Render Seaweed (Procedural sway using Sine)
        for &(x_pos, tallness) in &self.seaweed_seeds {
            for i in 0..tallness {
                let sway = (t_abs + (i as f64 * 0.5)).sin() * 1.5;
                let x = (x_pos as f64 + sway) as i32;
                let y = h as i32 - 1 - i;
                if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                    buf[y as usize][x as usize] = format!("{}\x1b[32m(\x1b[0m", DIM);
                }
            }
        }

        // 2. Render Bubbles
        for b in &self.bubbles {
            let x = b.x as i32;
            let y = b.y as i32;
            if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                buf[y as usize][x as usize] = "\x1b[94mo".to_string() + RESET;
            }
        }

        // 3. Render Fish
        for fish in &self.fish {
            let x_start = fish.x as i32;
            let y = fish.y as i32;
            let color = self.color_provider.get(t_abs, fish.seed);
            
            // Flip the ASCII if swimming left
            let display_body = if fish.direction < 0.0 {
                fish.body.chars().rev().map(|c| match c {
                    '<' => '>', '>' => '<', '(' => ')', ')' => '(', '{' => '}', '}' => '{',
                    _ => c
                }).collect::<String>()
            } else {
                fish.body.clone()
            };

            for (i, ch) in display_body.chars().enumerate() {
                let x = x_start + i as i32;
                if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
                    buf[y as usize][x as usize] = format!("{}{}{}", color, ch, RESET);
                }
            }
        }

        buf.iter().map(|row| row.join("")).collect::<Vec<_>>().join("\n")
    }
}