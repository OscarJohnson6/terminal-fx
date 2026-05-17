// ===== src/modes/volcano_islands.rs =====
//
// VolcanoIslandsMode
//
// ASCII/glyph island-chain simulation with multiple volcanoes.
// Each volcano has its own state machine:
//
//   Dormant -> Rumble -> Erupt -> Smoke -> Dormant
//
// Visual layers:
//   sky -> clouds -> ocean -> island/volcano shapes -> lava channels ->
//   eruption particles -> smoke/ash/debris
//
// This mode intentionally uses symbols instead of half-block pixels because
// volcanoes read well as terminal glyphs: /\ ▓ ▒ ≈ ~ * ✦ ● · ░

use crate::ansi::{rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VolcanoState {
    Dormant,
    Rumble,
    Erupt,
    Smoke,
}

struct Volcano {
    x: i32,
    base_y: i32,
    height: i32,
    width: i32,
    state: VolcanoState,
    timer: f64,
    next_timer: f64,
    lava_level: f64,
    seed: f64,
}

struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    ch: char,
    color: (u8, u8, u8),
    gravity: f64,
}

struct Cloud {
    x: f64,
    y: f64,
    speed: f64,
    width: usize,
}

pub struct VolcanoIslandsMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    volcanoes: Vec<Volcano>,
    particles: Vec<Particle>,
    clouds: Vec<Cloud>,
    last_dims: Option<(u16, u16)>,
}

impl VolcanoIslandsMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            volcanoes: Vec::new(),
            particles: Vec::new(),
            clouds: Vec::new(),
            last_dims: None,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();

        self.volcanoes.clear();
        self.particles.clear();
        self.clouds.clear();

        let w = width.max(1) as i32;
        let h = height.max(1) as i32;
        let sea_y = sea_level(height);

        let count = if w > 120 {
            5
        } else if w > 82 {
            4
        } else {
            3
        };

        for i in 0..count {
            let frac = (i as f64 + 0.55) / count as f64;
            let x = (frac * w as f64 + rng.random_range(-5.0..5.0)) as i32;
            let vh = rng
                .random_range((h / 5).max(4)..(h / 3).max(7))
                .max(5);
            let vw = rng.random_range(12..26).min(w.max(12));

            self.volcanoes.push(Volcano {
                x,
                base_y: sea_y,
                height: vh,
                width: vw,
                state: VolcanoState::Dormant,
                timer: 0.0,
                next_timer: rng.random_range(2.0..9.0),
                lava_level: 0.0,
                seed: rng.random_range(0.0..1000.0),
            });
        }

        let cloud_count = (w / 22).clamp(3, 8);
        for _ in 0..cloud_count {
            self.clouds.push(Cloud {
                x: rng.random_range(-40.0..w as f64 + 30.0),
                y: rng.random_range(1.0..7.0),
                speed: rng.random_range(0.5..2.0),
                width: rng.random_range(9..22),
            });
        }

        self.last_dims = Some((width, height));
    }

    fn spawn_particle(&mut self, x: f64, y: f64, kind: VolcanoState) {
        let mut rng = rand::rng();

        match kind {
            VolcanoState::Erupt => {
                let angle: f64 = rng.random_range(-2.72..-0.42);
                let spd = rng.random_range(10.0..30.0);
                let hot = rng.random_range(0..5);

                self.particles.push(Particle {
                    x,
                    y,
                    vx: angle.cos() * spd,
                    vy: angle.sin() * spd,
                    life: rng.random_range(0.8..2.4),
                    max_life: 2.4,
                    ch: match hot {
                        0 => '*',
                        1 => '✦',
                        2 => '●',
                        3 => '×',
                        _ => '·',
                    },
                    color: match hot {
                        0 => (255, 225, 90),
                        1 => (255, 145, 35),
                        2 => (215, 55, 28),
                        3 => (255, 90, 45),
                        _ => (130, 100, 82),
                    },
                    gravity: 18.0,
                });
            }
            VolcanoState::Smoke => {
                self.particles.push(Particle {
                    x: x + rng.random_range(-2.5..2.5),
                    y,
                    vx: rng.random_range(-3.0..3.0),
                    vy: rng.random_range(-6.0..-1.5),
                    life: rng.random_range(1.5..4.0),
                    max_life: 4.0,
                    ch: match rng.random_range(0..5) {
                        0 => '░',
                        1 => '▒',
                        2 => '·',
                        3 => 'o',
                        _ => '○',
                    },
                    color: (95, 92, 92),
                    gravity: -0.7,
                });
            }
            VolcanoState::Rumble => {
                self.particles.push(Particle {
                    x: x + rng.random_range(-3.0..3.0),
                    y,
                    vx: rng.random_range(-1.0..1.0),
                    vy: rng.random_range(-2.0..0.5),
                    life: rng.random_range(0.5..1.3),
                    max_life: 1.3,
                    ch: '·',
                    color: (210, 90, 45),
                    gravity: 2.0,
                });
            }
            VolcanoState::Dormant => {}
        }
    }
}

impl Mode for VolcanoIslandsMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.volcanoes.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = dt * self.speed;
        self.time += dt;

        let w = width.max(1) as f64;
        let mut rng = rand::rng();

        for cloud in &mut self.clouds {
            cloud.x += cloud.speed * dt;
            if cloud.x > w + cloud.width as f64 + 10.0 {
                cloud.x = -(cloud.width as f64) - 10.0;
            }
        }

        let mut spawn_requests: Vec<(f64, f64, VolcanoState)> = Vec::new();

        for v in &mut self.volcanoes {
            v.timer += dt;

            match v.state {
                VolcanoState::Dormant => {
                    v.next_timer -= dt;
                    v.lava_level = (v.lava_level - dt * 0.08).max(0.0);

                    if v.next_timer <= 0.0 {
                        v.state = VolcanoState::Rumble;
                        v.timer = 0.0;
                    }
                }
                VolcanoState::Rumble => {
                    v.lava_level = (v.lava_level + dt * 0.32).min(1.0);

                    if rng.random_range(0.0..1.0) < 0.22 {
                        spawn_requests.push((
                            v.x as f64,
                            (v.base_y - v.height) as f64,
                            VolcanoState::Smoke,
                        ));
                    }

                    if rng.random_range(0.0..1.0) < 0.14 {
                        spawn_requests.push((
                            v.x as f64,
                            (v.base_y - v.height + 1) as f64,
                            VolcanoState::Rumble,
                        ));
                    }

                    if v.timer > rng.random_range(1.1..2.4) {
                        v.state = VolcanoState::Erupt;
                        v.timer = 0.0;
                    }
                }
                VolcanoState::Erupt => {
                    v.lava_level = 1.0;

                    for _ in 0..4 {
                        spawn_requests.push((
                            v.x as f64,
                            (v.base_y - v.height - 1) as f64,
                            VolcanoState::Erupt,
                        ));
                    }

                    if rng.random_range(0.0..1.0) < 0.65 {
                        spawn_requests.push((
                            v.x as f64,
                            (v.base_y - v.height - 2) as f64,
                            VolcanoState::Smoke,
                        ));
                    }

                    if v.timer > rng.random_range(2.2..4.8) {
                        v.state = VolcanoState::Smoke;
                        v.timer = 0.0;
                    }
                }
                VolcanoState::Smoke => {
                    v.lava_level = (v.lava_level - dt * 0.18).max(0.18);

                    if rng.random_range(0.0..1.0) < 0.38 {
                        spawn_requests.push((
                            v.x as f64,
                            (v.base_y - v.height - 1) as f64,
                            VolcanoState::Smoke,
                        ));
                    }

                    if v.timer > rng.random_range(4.0..8.5) {
                        v.state = VolcanoState::Dormant;
                        v.timer = 0.0;
                        v.next_timer = rng.random_range(4.0..14.0);
                    }
                }
            }
        }

        for (x, y, kind) in spawn_requests {
            self.spawn_particle(x, y, kind);
        }

        let sea = sea_level(height) as f64;

        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vy += p.gravity * dt;
            p.life -= dt;

            if p.y > sea + 3.0 {
                p.life = 0.0;
            }
        }

        self.particles
            .retain(|p| p.life > 0.0 && p.x > -8.0 && p.x < w + 8.0 && p.y > -8.0);

        if self.particles.len() > 760 {
            let drop = self.particles.len() - 760;
            self.particles.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
        let mut buf = vec![vec![" ".to_string(); w]; h];

        draw_sky(&mut buf, w, h, self.time);
        draw_clouds(&mut buf, w, h, &self.clouds);
        draw_sea(&mut buf, w, h, self.time);

        for v in &self.volcanoes {
            draw_volcano(&mut buf, w, h, v, self.time);
        }

        for p in &self.particles {
            draw_particle(&mut buf, w, h, p);
        }

        let _ = self.color_provider.get(t_abs, self.time as i32);

        buf.into_iter()
            .map(|row| row.join(""))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn sea_level(height: u16) -> i32 {
    ((height as f64) * 0.73) as i32
}

fn in_bounds(w: usize, h: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < h as i32
}

fn draw_sky(buf: &mut Vec<Vec<String>>, w: usize, h: usize, time: f64) {
    let sea = sea_level(h as u16).max(0) as usize;

    for y in 0..sea.min(h) {
        let t = y as f64 / sea.max(1) as f64;
        let r = (38.0 + t * 70.0) as u8;
        let g = (80.0 + t * 70.0) as u8;
        let b = (135.0 + t * 60.0) as u8;

        for x in 0..w {
            let haze = ((x as f64 * 0.04 + time * 0.20).sin() + 1.0) * 0.5;
            buf[y][x] = format!(
                "{} {}",
                rgb(r, (g as f64 + haze * 8.0) as u8, b),
                RESET
            );
        }
    }

    // Distant sun/moon glow.
    let sx = (w as f64 * 0.82) as i32;
    let sy = (h as f64 * 0.15) as i32;
    for dy in -2..=2 {
        for dx in -4..=4 {
            if dx * dx + dy * dy * 4 <= 16 && in_bounds(w, h, sx + dx, sy + dy) {
                buf[(sy + dy) as usize][(sx + dx) as usize] =
                    format!("{}{}{}", rgb(255, 210, 90), '●', RESET);
            }
        }
    }
}

fn draw_clouds(buf: &mut Vec<Vec<String>>, w: usize, h: usize, clouds: &[Cloud]) {
    for c in clouds {
        let text = ["  .--.  ", " (    ) ", "  '--'  "];

        for (dy, row) in text.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                let x = c.x.round() as i32 + dx as i32;
                let y = c.y.round() as i32 + dy as i32;

                if ch != ' ' && in_bounds(w, h, x, y) {
                    buf[y as usize][x as usize] =
                        format!("{}{}{}", rgb(220, 225, 230), ch, RESET);
                }
            }
        }
    }
}

fn draw_sea(buf: &mut Vec<Vec<String>>, w: usize, h: usize, time: f64) {
    let sea = sea_level(h as u16).max(0) as usize;

    for y in sea..h {
        let depth = (y - sea) as f64 / (h - sea).max(1) as f64;
        for x in 0..w {
            let wave = ((x as f64 * 0.28 + time * 2.8 + y as f64 * 0.12).sin() + 1.0) * 0.5;
            let ch = if wave > 0.68 {
                '≈'
            } else if wave > 0.36 {
                '~'
            } else {
                '·'
            };

            let col = (
                (25.0 - depth * 10.0).max(5.0) as u8,
                (105.0 - depth * 30.0).max(30.0) as u8,
                (165.0 - depth * 50.0).max(70.0) as u8,
            );

            buf[y][x] = format!("{}{}{}", rgb(col.0, col.1, col.2), ch, RESET);
        }
    }
}

fn draw_volcano(buf: &mut Vec<Vec<String>>, w: usize, h: usize, v: &Volcano, time: f64) {
    let top_y = v.base_y - v.height;

    let shake = if matches!(v.state, VolcanoState::Rumble | VolcanoState::Erupt) {
        ((time * 18.0 + v.seed).sin()).round() as i32
    } else {
        0
    };

    // Mountain cone.
    for y in top_y..=v.base_y {
        let progress = (y - top_y) as f64 / v.height.max(1) as f64;
        let half_width = (progress * v.width as f64 * 0.5).max(1.0) as i32;

        for dx in -half_width..=half_width {
            let x = v.x + dx + shake;
            if !in_bounds(w, h, x, y) {
                continue;
            }

            let edge = dx.abs() == half_width;
            let crater = y <= top_y + 1 && dx.abs() <= 2;
            let lava_channel = dx.abs() <= 1 && v.lava_level > 0.15 && y > top_y + 1;
            let side_lava =
                v.lava_level > 0.6 && ((dx + y + v.seed as i32) % 11 == 0) && y > top_y + 4;

            let (ch, col) = if crater && v.lava_level > 0.2 {
                ('█', (255, 105, 35))
            } else if lava_channel && ((y as f64 * 0.7 + time * 5.0).sin() > 0.0) {
                ('│', (255, 125, 35))
            } else if side_lava {
                ('╲', (255, 100, 30))
            } else if edge {
                if dx < 0 {
                    ('/', (82, 66, 55))
                } else {
                    ('\\', (82, 66, 55))
                }
            } else {
                let rock = if (x + y) % 5 == 0 { '▒' } else { '▓' };
                (rock, (70, 58, 50))
            };

            buf[y as usize][x as usize] = format!("{}{}{}", rgb(col.0, col.1, col.2), ch, RESET);
        }
    }

    // Island base / green edge.
    for dx in -(v.width / 2 + 6)..=(v.width / 2 + 6) {
        let x = v.x + dx;
        if in_bounds(w, h, x, v.base_y + 1) {
            buf[(v.base_y + 1) as usize][x as usize] =
                format!("{}{}{}", rgb(42, 90, 45), '▄', RESET);
        }
    }

    // Rumble warning sparks near crater.
    if matches!(v.state, VolcanoState::Rumble) {
        let pulse = ((time * 7.0 + v.seed).sin() + 1.0) * 0.5;
        if pulse > 0.45 {
            for dx in -2..=2 {
                if in_bounds(w, h, v.x + dx, top_y - 1) {
                    buf[(top_y - 1) as usize][(v.x + dx) as usize] =
                        format!("{}{}{}", rgb(255, 150, 55), '·', RESET);
                }
            }
        }
    }
}

fn draw_particle(buf: &mut Vec<Vec<String>>, w: usize, h: usize, p: &Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);
    let col = (
        (p.color.0 as f64 * fade) as u8,
        (p.color.1 as f64 * fade) as u8,
        (p.color.2 as f64 * fade) as u8,
    );

    let x = p.x.round() as i32;
    let y = p.y.round() as i32;

    if in_bounds(w, h, x, y) {
        buf[y as usize][x as usize] = format!("{}{}{}", rgb(col.0, col.1, col.2), p.ch, RESET);
    }
}
