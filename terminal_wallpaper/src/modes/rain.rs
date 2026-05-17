// ===== src/modes/rain.rs =====
//
// Rain columns, splash particles, rolling thunder flashes, and forked lightning.

use crate::ansi::{rgb, RESET};
use crate::color::{ColorProvider, ColorMode};
use crate::mode_base::Mode;
use rand::RngExt;

struct Droplet {
    y:        f64,
    speed:    f64,
    char_idx: usize,
}

struct Splash {
    col:   usize,
    row:   usize,
    timer: f64,
}

struct Lightning {
    points:   Vec<(i32, i32)>,
    age:      f64,
    duration: f64,
    col:      usize,
}

pub struct RainMode {
    speed:               f64,
    color:               ColorProvider,
    columns:             Vec<Vec<Droplet>>,
    splashes:            Vec<Splash>,
    bolts:               Vec<Lightning>,
    lightning_timer:     f64,
    lightning_interval:  f64,
    thunder_glow:        f64,
    bg_flash:            f64,
    initialized:         bool,
}

impl RainMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self {
            speed, color,
            columns: Vec::new(), splashes: Vec::new(), bolts: Vec::new(),
            lightning_timer: 0.0, lightning_interval: 0.0,
            thunder_glow: 0.0, bg_flash: 0.0,
            initialized: false,
        }
    }

    fn init(&mut self, w: usize, h: usize) {
        let mut rng = rand::rng();
        self.columns = (0..w).map(|_| {
            let col_speed = rng.random_range(8.0..22.0) * self.speed;
            let count     = rng.random_range(1usize..5);
            (0..count).map(|i| Droplet {
                y:        rng.random_range(0.0..h as f64) - i as f64 * (h as f64 / count as f64),
                speed:    col_speed + rng.random_range(-2.0..2.0) * self.speed,
                char_idx: rng.random_range(0..3),
            }).collect()
        }).collect();

        self.lightning_interval = rng.random_range(3.5..8.0) / self.speed.max(0.3);
        self.lightning_timer    = rng.random_range(1.0..3.0);
        self.initialized        = true;
    }

    fn spawn_lightning(&mut self, w: usize, h: usize, rng: &mut impl RngExt) {
        let start_col = rng.random_range(4..w.saturating_sub(4));
        let mut pts   = Vec::new();
        let mut col   = start_col as i32;
        for row in 0..h as i32 {
            pts.push((col, row));
            if rng.random_bool(0.08) && row < h as i32 - 3 {
                let fork_dir = if rng.random_bool(0.5) { 1i32 } else { -1 };
                for fr in row..(row + rng.random_range(3..8)).min(h as i32) {
                    let fc = (col + fork_dir * (fr - row)).clamp(0, w as i32 - 1);
                    pts.push((fc, fr));
                }
            }
            col = (col + rng.random_range(-2i32..=2)).clamp(0, w as i32 - 1);
        }
        let duration = rng.random_range(0.08..0.18);
        self.bolts.push(Lightning { points: pts, age: 0.0, duration, col: start_col });
        self.thunder_glow       = 1.0;
        self.bg_flash           = 1.0;
        self.lightning_interval = rng.random_range(2.5..9.0) / self.speed.max(0.3);
        self.lightning_timer    = self.lightning_interval;
    }
}

impl Mode for RainMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t: f64) {
        let w = width  as usize;
        let h = height as usize;
        if !self.initialized { self.init(w, h); return; }
        let dt = dt * self.speed;
        let mut rng = rand::rng();

        // ── Rain drops ────────────────────────────────────────────────────
        for (col, drops) in self.columns.iter_mut().enumerate() {
            for drop in drops.iter_mut() {
                drop.y += drop.speed * dt;
                if drop.y >= h as f64 {
                    drop.y -= h as f64;
                    self.splashes.push(Splash { col, row: h - 1, timer: 0.0 });
                }
            }
        }

        // ── Splashes ──────────────────────────────────────────────────────
        for s in &mut self.splashes { s.timer += dt; }
        self.splashes.retain(|s| s.timer < 0.38);

        // ── Lightning ─────────────────────────────────────────────────────
        self.lightning_timer -= dt;
        if self.lightning_timer <= 0.0 {
            self.spawn_lightning(w, h, &mut rng);
        }
        for b in &mut self.bolts { b.age += dt; }
        self.bolts.retain(|b| b.age < b.duration);

        self.thunder_glow = (self.thunder_glow - dt * 1.8).max(0.0);
        self.bg_flash     = (self.bg_flash     - dt * 8.0).max(0.0);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width  as usize;
        let h = height as usize;
        let mut buf = vec![vec![String::from(" "); w]; h];

        // ── Sky background flash from lightning ────────────────────────────
        let flash_add = (self.bg_flash * 55.0) as u8;
        if self.bg_flash > 0.02 || self.thunder_glow > 0.02 {
            let r = 18 + flash_add;
            let g = 22 + flash_add;
            let b = 32 + flash_add;
            let thunder = (self.thunder_glow * 18.0) as u8;
            let bands = h / 3;
            for row in 0..bands {
                let fade = 1.0 - row as f64 / bands as f64;
                let rr = ((r as f64) + (thunder as f64) * fade) as u8;
                let gg = ((g as f64) + (thunder as f64) * fade) as u8;
                let bb = ((b as f64) + (thunder as f64) * fade) as u8;
                for col in 0..w {
                    buf[row][col] = format!("{} {}", rgb(rr, gg, bb), RESET);
                }
            }
        }

        // ── Rain drops ─────────────────────────────────────────────────────
        const CHARS: &[char] = &['│', '╎', '╷'];
        for (col, drops) in self.columns.iter().enumerate() {
            for drop in drops {
                let row = drop.y as usize;
                if row < h {
                    let ch = CHARS[drop.char_idx % CHARS.len()];
                    let (r, g, b) = match self.color.mode {
                        ColorMode::Matrix => (0, 180, 60),
                        ColorMode::Sunset => (140, 100, 90),
                        ColorMode::Ocean  => (60, 130, 220),
                        _                 => {
                            let v = (((col as f64 * 0.3 + t_abs * 2.1).sin() + 1.0) * 20.0) as u8 + 90;
                            (v.saturating_sub(40), v.saturating_sub(10), v.saturating_add(30))
                        }
                    };
                    buf[row][col] = format!("{}{}{}", rgb(r, g, b), ch, RESET);
                }
            }
        }

        // ── Splashes ──────────────────────────────────────────────────────
        const SPLASH_CH: &[char] = &['·', '˜', '~', '`'];
        for s in &self.splashes {
            let frame  = (s.timer / 0.38 * 4.0) as usize;
            let ch     = SPLASH_CH[frame.min(SPLASH_CH.len() - 1)];
            let spread = frame as i32;
            for dx in [-spread, spread] {
                let pc = s.col as i32 + dx;
                if pc >= 0 && pc < w as i32 {
                    let fade = 1.0 - s.timer / 0.38;
                    let v    = (fade * 140.0) as u8 + 30;
                    buf[s.row][pc as usize] =
                        format!("{}{}{}", rgb(v, v.saturating_add(20), v.saturating_add(40)), ch, RESET);
                }
            }
        }

        // ── Lightning bolts ───────────────────────────────────────────────
        for bolt in &self.bolts {
            let life    = 1.0 - bolt.age / bolt.duration;
            let flicker = if bolt.age < 0.04 { 1.0 } else { life * 0.7 + 0.3 };
            let val     = (flicker * 255.0) as u8;
            let blue    = ((flicker * 200.0) as u8).saturating_add(55);
            for &(bc, br) in &bolt.points {
                if bc >= 0 && bc < w as i32 && br >= 0 && br < h as i32 {
                    let col = rgb(val, val, blue);
                    buf[br as usize][bc as usize] = format!("{}║{}", col, RESET);
                }
            }

            let gcol = bolt.col as i32;
            let gw   = 2i32;
            let gval = (flicker * 35.0) as u8;
            for row in 0..h {
                for dx in -gw..=gw {
                    let pc = gcol + dx;
                    if pc >= 0 && pc < w as i32 && dx != 0 && buf[row][pc as usize] == " " {
                        buf[row][pc as usize] =
                            format!("{} {}", rgb(gval, gval, gval.saturating_add(20)), RESET);
                    }
                }
            }
        }

        buf.into_iter().map(|r| r.join("")).collect::<Vec<_>>().join("\n")
    }
}