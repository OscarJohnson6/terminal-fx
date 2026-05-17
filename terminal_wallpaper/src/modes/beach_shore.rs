// ===== src/modes/beach_shore.rs =====
//
// BeachShoreMode
//
// Symbol-heavy shoreline mode.
// Waves move in and out, random objects wash ashore, birds peck/carry them,
// and the tide eventually clears the scene so new stuff appears.
//
// Visual style:
//   Old-school terminal glyph scene, closer to TreeMode than half-block pixel art.
//
// Layers:
//   sky -> clouds -> ocean -> moving tide foam -> beach -> washed-up objects -> birds/crabs

use crate::ansi::{rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

#[derive(Clone, Copy)]
enum ShoreThing {
    Shell,
    Bottle,
    Starfish,
    Driftwood,
    Kelp,
    Coin,
    CrabSnack,
}

impl ShoreThing {
    fn glyph(self) -> char {
        match self {
            ShoreThing::Shell => '◖',
            ShoreThing::Bottle => '▱',
            ShoreThing::Starfish => '✶',
            ShoreThing::Driftwood => '=',
            ShoreThing::Kelp => '≈',
            ShoreThing::Coin => '○',
            ShoreThing::CrabSnack => '•',
        }
    }

    fn color(self) -> (u8, u8, u8) {
        match self {
            ShoreThing::Shell => (245, 210, 165),
            ShoreThing::Bottle => (90, 205, 180),
            ShoreThing::Starfish => (255, 125, 90),
            ShoreThing::Driftwood => (130, 90, 55),
            ShoreThing::Kelp => (45, 145, 75),
            ShoreThing::Coin => (245, 200, 80),
            ShoreThing::CrabSnack => (235, 180, 120),
        }
    }
}

#[derive(Clone)]
struct WashedItem {
    x: f64,
    y: f64,
    kind: ShoreThing,
    age: f64,
    carried: bool,
    bob_phase: f64,
}

#[derive(Clone, Copy)]
struct Bird {
    x: f64,
    y: f64,
    vx: f64,
    target_item: Option<usize>,
    carry_timer: f64,
    wing_phase: f64,
}

#[derive(Clone, Copy)]
struct Crab {
    x: f64,
    y: f64,
    vx: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Cloud {
    x: f64,
    y: f64,
    speed: f64,
    width: usize,
}

pub struct BeachShoreMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    items: Vec<WashedItem>,
    birds: Vec<Bird>,
    crabs: Vec<Crab>,
    clouds: Vec<Cloud>,
    spawn_timer: f64,
    last_dims: Option<(u16, u16)>,
}

impl BeachShoreMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        let mut rng = rand::rng();

        let birds = (0..5)
            .map(|i| Bird {
                x: rng.random_range(0.0..120.0),
                y: rng.random_range(3.0..10.0),
                vx: if i % 2 == 0 {
                    rng.random_range(7.0..15.0)
                } else {
                    -rng.random_range(7.0..15.0)
                },
                target_item: None,
                carry_timer: 0.0,
                wing_phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let crabs = (0..4)
            .map(|i| Crab {
                x: rng.random_range(0.0..120.0),
                y: 20.0,
                vx: if i % 2 == 0 { 3.0 } else { -3.0 },
                phase: rng.random_range(0.0..TAU),
            })
            .collect();

        let clouds = (0..5)
            .map(|_| Cloud {
                x: rng.random_range(-60.0..140.0),
                y: rng.random_range(1.0..7.0),
                speed: rng.random_range(1.0..3.0),
                width: rng.random_range(10..24),
            })
            .collect();

        Self {
            speed,
            color_provider,
            time: 0.0,
            items: Vec::new(),
            birds,
            crabs,
            clouds,
            spawn_timer: 0.0,
            last_dims: None,
        }
    }

    fn reset_for_size(&mut self, width: u16, height: u16) {
        self.items.clear();

        let mut rng = rand::rng();
        let h = height.max(1) as f64;
        let beach_y = beach_start(height) as f64;

        for crab in &mut self.crabs {
            crab.x = rng.random_range(0.0..width.max(1) as f64);
            crab.y = rng.random_range(beach_y + 2.0..(h - 1.0).max(beach_y + 3.0));
        }

        self.last_dims = Some((width, height));
    }

    fn spawn_item(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();

        let kind = match rng.random_range(0..7) {
            0 => ShoreThing::Shell,
            1 => ShoreThing::Bottle,
            2 => ShoreThing::Starfish,
            3 => ShoreThing::Driftwood,
            4 => ShoreThing::Kelp,
            5 => ShoreThing::Coin,
            _ => ShoreThing::CrabSnack,
        };

        let w = width.max(1) as f64;
        let beach = beach_start(height) as f64;
        let tide = tide_line(height, self.time);

        self.items.push(WashedItem {
            x: rng.random_range(2.0..(w - 2.0).max(3.0)),
            y: rng.random_range(tide + 1.0..(beach + 7.0).max(tide + 2.0)),
            kind,
            age: 0.0,
            carried: false,
            bob_phase: rng.random_range(0.0..TAU),
        });

        if self.items.len() > 28 {
            self.items.remove(0);
        }
    }
}

impl Mode for BeachShoreMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) {
            self.reset_for_size(width, height);
        }

        let dt = dt * self.speed;
        self.time += dt;

        let w = width.max(1) as f64;
        let h = height.max(1) as f64;
        let tide = tide_line(height, self.time);
        let beach = beach_start(height) as f64;

        for c in &mut self.clouds {
            c.x += c.speed * dt;
            if c.x > w + c.width as f64 {
                c.x = -(c.width as f64) - 3.0;
            }
        }

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            self.spawn_item(width, height);
            self.spawn_timer = rand::rng().random_range(0.8..2.4);
        }

        for item in &mut self.items {
            item.age += dt;

            // Tide pushes items upward/downward gently.
            let wave_pull = ((self.time * 1.7 + item.bob_phase).sin()) * 0.018;
            item.x += ((self.time * 0.8 + item.bob_phase).cos()) * dt * 0.16;
            item.y += wave_pull;

            // If the tide reaches an item, it may get dragged away.
            if item.y <= tide + 0.8 && rand::rng().random_range(0.0..1.0) < 0.04 {
                item.y -= dt * 4.5;
            }

            item.x = item.x.clamp(0.0, w - 1.0);
        }

        // Birds target visible shore objects, then carry them away.
        for bird in &mut self.birds {
            bird.wing_phase += dt * 7.0;

            if bird.carry_timer > 0.0 {
                bird.carry_timer -= dt;
                bird.x += bird.vx * dt;
                bird.y -= dt * 2.2;

                if bird.x < -8.0 || bird.x > w + 8.0 || bird.y < -4.0 {
                    bird.carry_timer = 0.0;
                    bird.target_item = None;
                    bird.y = rand::rng().random_range(3.0..10.0);
                    bird.x = if bird.vx > 0.0 { -4.0 } else { w + 4.0 };
                }

                continue;
            }

            if let Some(idx) = bird.target_item {
                if let Some(item) = self.items.get(idx) {
                    let dx = item.x - bird.x;
                    let dy = item.y - bird.y;
                    let dist = (dx * dx + dy * dy).sqrt().max(0.01);

                    bird.x += dx / dist * dt * 13.0;
                    bird.y += dy / dist * dt * 6.0;

                    if dist < 1.6 {
                        bird.carry_timer = 2.0;
                    }
                } else {
                    bird.target_item = None;
                }
            } else {
                bird.x += bird.vx * dt;
                bird.y += (self.time * 1.2 + bird.wing_phase).sin() * dt * 0.35;

                if bird.x > w + 4.0 {
                    bird.x = -4.0;
                } else if bird.x < -4.0 {
                    bird.x = w + 4.0;
                }

                if rand::rng().random_range(0.0..1.0) < 0.018 && !self.items.is_empty() {
                    let idx = rand::rng().random_range(0..self.items.len());
                    if self.items[idx].y >= beach - 1.0 {
                        bird.target_item = Some(idx);
                    }
                }
            }
        }

        // Remove carried items.
        let mut remove_indices = Vec::new();
        for bird in &self.birds {
            if bird.carry_timer > 1.6 {
                if let Some(idx) = bird.target_item {
                    remove_indices.push(idx);
                }
            }
        }
        remove_indices.sort_unstable();
        remove_indices.dedup();
        for idx in remove_indices.into_iter().rev() {
            if idx < self.items.len() {
                self.items.remove(idx);
            }
        }

        // Crabs wander and sometimes eat/remove small things.
        let mut eaten = Vec::new();
        for crab in &mut self.crabs {
            crab.phase += dt * 5.0;
            crab.x += crab.vx * dt;

            if crab.x < 1.0 {
                crab.x = 1.0;
                crab.vx = crab.vx.abs();
            } else if crab.x > w - 2.0 {
                crab.x = w - 2.0;
                crab.vx = -crab.vx.abs();
            }

            if rand::rng().random_range(0.0..1.0) < 0.01 {
                crab.vx = -crab.vx;
            }

            for (i, item) in self.items.iter().enumerate() {
                let dx = item.x - crab.x;
                let dy = item.y - crab.y;
                if (dx * dx + dy * dy).sqrt() < 1.8 {
                    if matches!(item.kind, ShoreThing::CrabSnack | ShoreThing::Shell | ShoreThing::Coin) {
                        eaten.push(i);
                    }
                }
            }
        }
        eaten.sort_unstable();
        eaten.dedup();
        for idx in eaten.into_iter().rev() {
            if idx < self.items.len() {
                self.items.remove(idx);
            }
        }

        // Old or washed-away objects vanish so the shore keeps refreshing.
        self.items.retain(|item| item.age < 28.0 && item.y > tide - 5.0 && item.y < h);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
        let mut buf = vec![vec![" ".to_string(); w]; h];

        draw_sky(&mut buf, w, h, self.time);
        draw_clouds(&mut buf, w, h, &self.clouds);
        draw_ocean(&mut buf, w, h, self.time);
        draw_beach(&mut buf, w, h, self.time);

        for item in &self.items {
            let x = item.x.round() as i32;
            let y = item.y.round() as i32;
            if in_bounds(w, h, x, y) {
                let (r, g, b) = item.kind.color();
                buf[y as usize][x as usize] =
                    format!("{}{}{}", rgb(r, g, b), item.kind.glyph(), RESET);
            }
        }

        for crab in &self.crabs {
            draw_crab(&mut buf, w, h, crab, self.time);
        }

        for bird in &self.birds {
            draw_bird(&mut buf, w, h, bird);
        }

        let _ = self.color_provider.get(t_abs, self.time as i32);

        buf.into_iter()
            .map(|row| row.join(""))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn beach_start(height: u16) -> usize {
    ((height as f64) * 0.64) as usize
}

fn tide_line(height: u16, time: f64) -> f64 {
    let beach = beach_start(height) as f64;
    beach - 1.5 + (time * 0.75).sin() * 3.0 + (time * 1.7).sin() * 0.7
}

fn in_bounds(w: usize, h: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < h as i32
}

fn draw_sky(buf: &mut Vec<Vec<String>>, w: usize, h: usize, time: f64) {
    let sky_h = beach_start(h as u16).saturating_sub(5);

    for y in 0..sky_h.min(h) {
        let t = y as f64 / sky_h.max(1) as f64;
        let r = (75.0 + t * 65.0) as u8;
        let g = (150.0 + t * 55.0) as u8;
        let b = (215.0 + t * 20.0) as u8;

        for x in 0..w {
            let shimmer = ((x as f64 * 0.05 + time * 0.3).sin() + 1.0) * 0.5;
            let rr = (r as f64 + shimmer * 5.0) as u8;
            buf[y][x] = format!("{} {}", rgb(rr, g, b), RESET);
        }
    }

    // Sun.
    let sx = (w as f64 * 0.78) as i32;
    let sy = (h as f64 * 0.12) as i32;
    for dy in -2..=2 {
        for dx in -4..=4 {
            if dx * dx + dy * dy * 4 <= 16 {
                if in_bounds(w, h, sx + dx, sy + dy) {
                    buf[(sy + dy) as usize][(sx + dx) as usize] =
                        format!("{}☀{}", rgb(255, 225, 90), RESET);
                }
            }
        }
    }
}

fn draw_clouds(buf: &mut Vec<Vec<String>>, w: usize, h: usize, clouds: &[Cloud]) {
    for c in clouds {
        let x = c.x.round() as i32;
        let y = c.y.round() as i32;
        let text = [" .--. ", "(    )", " '--' "];

        for (dy, line) in text.iter().enumerate() {
            for (dx, ch) in line.chars().enumerate() {
                let px = x + dx as i32;
                let py = y + dy as i32;
                if ch != ' ' && in_bounds(w, h, px, py) {
                    buf[py as usize][px as usize] =
                        format!("{}{}{}", rgb(238, 245, 250), ch, RESET);
                }
            }
        }
    }
}

fn draw_ocean(buf: &mut Vec<Vec<String>>, w: usize, h: usize, time: f64) {
    let ocean_start = beach_start(h as u16).saturating_sub(10);
    let beach = beach_start(h as u16);
    let tide = tide_line(h as u16, time);

    for y in ocean_start..beach.min(h) {
        let depth = (y - ocean_start) as f64 / (beach - ocean_start).max(1) as f64;
        let base = if y as f64 >= tide {
            (105, 220, 230)
        } else {
            (35, 130, 190)
        };

        for x in 0..w {
            let wave = ((x as f64 * 0.36 + time * 3.3 + y as f64 * 0.22).sin() + 1.0) * 0.5;
            let ch = if y as f64 >= tide {
                match wave {
                    v if v > 0.72 => '≈',
                    v if v > 0.42 => '~',
                    _ => '·',
                }
            } else {
                match wave {
                    v if v > 0.72 => '≋',
                    v if v > 0.42 => '≈',
                    _ => '~',
                }
            };

            let r = (base.0 as f64 * (1.0 - depth * 0.15)) as u8;
            let g = (base.1 as f64 * (1.0 - depth * 0.08)) as u8;
            let b = base.2;
            buf[y][x] = format!("{}{}{}", rgb(r, g, b), ch, RESET);
        }
    }
}

fn draw_beach(buf: &mut Vec<Vec<String>>, w: usize, h: usize, time: f64) {
    let beach = beach_start(h as u16);

    for y in beach..h {
        let depth = (y - beach) as f64 / (h - beach).max(1) as f64;
        for x in 0..w {
            let grain = ((x as f64 * 1.7 + y as f64 * 0.9 + time * 0.15).sin() + 1.0) * 0.5;
            let ch = if grain > 0.92 {
                '·'
            } else if grain > 0.82 {
                '\''
            } else {
                ' '
            };

            let r = (214.0 - depth * 35.0 + grain * 8.0) as u8;
            let g = (178.0 - depth * 30.0 + grain * 6.0) as u8;
            let b = (112.0 - depth * 20.0) as u8;

            buf[y][x] = format!("{}{}{}", rgb(r, g, b), ch, RESET);
        }
    }
}

fn draw_bird(buf: &mut Vec<Vec<String>>, w: usize, h: usize, bird: &Bird) {
    let x = bird.x.round() as i32;
    let y = bird.y.round() as i32;
    let flap = bird.wing_phase.sin() > 0.0;

    let glyphs = if bird.vx >= 0.0 {
        if flap { ['\\', 'v', '/'] } else { ['-', 'v', '-'] }
    } else if flap {
        ['\\', 'v', '/']
    } else {
        ['-', 'v', '-']
    };

    for (i, ch) in glyphs.iter().enumerate() {
        let px = x + i as i32 - 1;
        if in_bounds(w, h, px, y) {
            buf[y as usize][px as usize] = format!("{}{}{}", rgb(35, 35, 40), ch, RESET);
        }
    }

    if bird.carry_timer > 0.0 && in_bounds(w, h, x, y + 1) {
        buf[(y + 1) as usize][x as usize] = format!("{}{}{}", rgb(230, 180, 120), '•', RESET);
    }
}

fn draw_crab(buf: &mut Vec<Vec<String>>, w: usize, h: usize, crab: &Crab, time: f64) {
    let x = crab.x.round() as i32;
    let y = crab.y.round() as i32;
    let step = (time * 6.0 + crab.phase).sin() > 0.0;

    let crab_text = if step { "<(o_o)>" } else { "^(o_o)^" };

    for (i, ch) in crab_text.chars().enumerate() {
        let px = x + i as i32 - 3;
        if in_bounds(w, h, px, y) {
            buf[y as usize][px as usize] = format!("{}{}{}", rgb(210, 75, 55), ch, RESET);
        }
    }
}
