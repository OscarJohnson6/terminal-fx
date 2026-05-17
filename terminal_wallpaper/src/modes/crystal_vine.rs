// ===== src/modes/crystal_tower_vine.rs =====
//
// CrystalVineMode
//
// Smooth half-block remake of the Crystal Vine idea.
//
// Scene loop:
//   1. A stone tower stands in a dark garden.
//   2. A glowing crystalline vine grows up the tower.
//   3. Crystal flowers bloom along the vine.
//   4. A small figure arrives with a torch.
//   5. The vine burns/shatters into sparks and glass fragments.
//   6. Ash falls, the tower remains, and a new crystal seed forms.
//
// This version intentionally does NOT copy TreeMode's segment-growth look.
// It uses a pixel buffer, soft circles, glow, tower geometry, particles,
// and a stage machine.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Seed,
    Growing,
    Bloom,
    FigureArrives,
    Shatter,
    Ash,
}

#[derive(Clone, Copy)]
struct VineNode {
    x: f64,
    y: f64,
    t: f64,
    side: f64,
    bloom: bool,
    phase: f64,
}

#[derive(Clone, Copy)]
enum ParticleKind {
    Spark,
    Crystal,
    Smoke,
    Ash,
}

#[derive(Clone, Copy)]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max_life: f64,
    color: Rgb,
    kind: ParticleKind,
}

pub struct CrystalVineMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    stage_time: f64,
    stage: Stage,
    nodes: Vec<VineNode>,
    particles: Vec<Particle>,
    figure_x: f64,
    last_dims: Option<(u16, u16)>,
    seed_phase: f64,
}

impl CrystalVineMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            stage_time: 0.0,
            stage: Stage::Seed,
            nodes: Vec::new(),
            particles: Vec::new(),
            figure_x: -20.0,
            last_dims: None,
            seed_phase: 0.0,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        self.nodes.clear();
        self.particles.clear();

        let mut rng = rand::rng();

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        let tower = tower_rect(w, ph);
        let base_x = tower.0 + tower.2 * 0.50;

        let node_count = 90;
        for i in 0..node_count {
            let t = i as f64 / (node_count - 1) as f64;

            // Vine climbs upward while wrapping around the tower.
            let y = tower.1 + tower.3 - t * tower.3 * 0.95;
            let spiral = (t * TAU * 4.4 + rng.random_range(-0.15..0.15)).sin();
            let side = spiral;

            let x = base_x + spiral * tower.2 * 0.34
                + (t * TAU * 1.7).cos() * tower.2 * 0.04;

            let bloom = i % 9 == 0 && i > 10;

            self.nodes.push(VineNode {
                x,
                y,
                t,
                side,
                bloom,
                phase: rng.random_range(0.0..TAU),
            });
        }

        self.figure_x = -12.0;
        self.seed_phase = rng.random_range(0.0..TAU);
        self.stage = Stage::Seed;
        self.stage_time = 0.0;
        self.last_dims = Some((width, height));
    }

    fn next_stage(&mut self, width: u16, height: u16) {
        self.stage_time = 0.0;

        self.stage = match self.stage {
            Stage::Seed => Stage::Growing,
            Stage::Growing => Stage::Bloom,
            Stage::Bloom => {
                self.figure_x = -12.0;
                Stage::FigureArrives
            }
            Stage::FigureArrives => {
                self.spawn_shatter(width, height);
                Stage::Shatter
            }
            Stage::Shatter => Stage::Ash,
            Stage::Ash => {
                self.reset_scene(width, height);
                Stage::Seed
            }
        };
    }

    fn visible_amount(&self) -> f64 {
        match self.stage {
            Stage::Seed => 0.02,
            Stage::Growing => (self.stage_time / 9.0).clamp(0.0, 1.0),
            Stage::Bloom | Stage::FigureArrives => 1.0,
            Stage::Shatter => (1.0 - self.stage_time / 1.2).clamp(0.0, 1.0),
            Stage::Ash => 0.0,
        }
    }

    fn spawn_shatter(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        let tower = tower_rect(w, ph);

        for node in &self.nodes {
            if rng.random_range(0.0..1.0) < 0.72 {
                let outward = if node.side >= 0.0 { 1.0 } else { -1.0 };
                let speed = rng.random_range(7.0..28.0);

                self.particles.push(Particle {
                    x: node.x,
                    y: node.y,
                    vx: outward * speed + rng.random_range(-5.0..5.0),
                    vy: rng.random_range(-22.0..8.0),
                    life: rng.random_range(0.8..2.3),
                    max_life: 2.3,
                    color: match rng.random_range(0..4) {
                        0 => (140, 255, 230),
                        1 => (210, 150, 255),
                        2 => (255, 110, 180),
                        _ => (255, 225, 120),
                    },
                    kind: if rng.random_range(0..3) == 0 {
                        ParticleKind::Spark
                    } else {
                        ParticleKind::Crystal
                    },
                });
            }
        }

        // Smoke/fire source near the figure's torch contact point.
        let contact_x = tower.0 + tower.2 * 0.25;
        let contact_y = tower.1 + tower.3 * 0.65;
        for _ in 0..160 {
            self.particles.push(Particle {
                x: contact_x + rng.random_range(-4.0..4.0),
                y: contact_y + rng.random_range(-8.0..8.0),
                vx: rng.random_range(-7.0..7.0),
                vy: rng.random_range(-18.0..-2.0),
                life: rng.random_range(0.6..2.7),
                max_life: 2.7,
                color: match rng.random_range(0..4) {
                    0 => (255, 190, 75),
                    1 => (255, 95, 45),
                    2 => (125, 115, 118),
                    _ => (75, 70, 78),
                },
                kind: if rng.random_range(0..2) == 0 {
                    ParticleKind::Smoke
                } else {
                    ParticleKind::Spark
                },
            });
        }
    }
}

impl Mode for CrystalVineMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.nodes.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = dt * self.speed;
        self.time += dt;
        self.stage_time += dt;

        match self.stage {
            Stage::Seed => {
                if self.stage_time > 1.7 {
                    self.next_stage(width, height);
                }
            }
            Stage::Growing => {
                if self.stage_time > 9.5 {
                    self.next_stage(width, height);
                }
            }
            Stage::Bloom => {
                if self.stage_time > 5.0 {
                    self.next_stage(width, height);
                }
            }
            Stage::FigureArrives => {
                let w = width.max(1) as f64;
                let ph = height.max(1) as f64 * 2.0;
                let tower = tower_rect(w, ph);

                let target = tower.0 - 7.0;
                self.figure_x += dt * 18.0;

                if self.figure_x >= target || self.stage_time > 5.5 {
                    self.next_stage(width, height);
                }
            }
            Stage::Shatter => {
                if self.stage_time > 2.4 {
                    self.next_stage(width, height);
                }
            }
            Stage::Ash => {
                if self.stage_time > 3.2 {
                    self.next_stage(width, height);
                }
            }
        }

        let ph = height.max(1) as f64 * 2.0;
        let ground = ph - 3.0;
        let mut rng = rand::rng();

        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            match p.kind {
                ParticleKind::Spark => {
                    p.vy += 14.0 * dt;
                    p.vx *= 0.985;
                }
                ParticleKind::Crystal => {
                    p.vy += 22.0 * dt;
                    p.vx *= 0.992;
                }
                ParticleKind::Smoke => {
                    p.vy -= 1.6 * dt;
                    p.vx += (self.time * 2.0 + p.y * 0.05).sin() * dt * 2.0;
                }
                ParticleKind::Ash => {
                    p.vy += 3.0 * dt;
                    p.vx += (self.time * 1.4 + p.x * 0.04).sin() * dt * 1.2;
                }
            }

            if p.y > ground {
                p.y = ground;
                p.vy *= -0.15;
                p.vx *= 0.4;
            }

            p.life -= dt;
        }

        self.particles.retain(|p| p.life > 0.0 && p.y > -12.0);

        // During ash stage, let little dust fall where the vine used to be.
        if self.stage == Stage::Ash && rng.random_range(0.0..1.0) < 0.55 {
            let w = width.max(1) as f64;
            let ph = height.max(1) as f64 * 2.0;
            let tower = tower_rect(w, ph);
            self.particles.push(Particle {
                x: tower.0 + rng.random_range(0.0..tower.2),
                y: tower.1 + rng.random_range(0.0..tower.3 * 0.5),
                vx: rng.random_range(-0.8..0.8),
                vy: rng.random_range(1.0..5.0),
                life: rng.random_range(0.8..1.8),
                max_life: 1.8,
                color: (110, 105, 110),
                kind: ParticleKind::Ash,
            });
        }

        if self.particles.len() > 900 {
            let drop = self.particles.len() - 900;
            self.particles.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_background(&mut pix, w, ph, self.time);
        paint_moon(&mut pix, w, ph, self.time);

        let tower = tower_rect(w as f64, ph as f64);
        paint_tower(&mut pix, w, ph, tower, self.time);

        paint_seed(&mut pix, w, ph, tower, self.stage, self.stage_time, self.seed_phase);

        let amount = self.visible_amount();
        if amount > 0.0 {
            paint_vine(&mut pix, w, ph, &self.nodes, amount, self.stage, self.time);
        }

        if matches!(self.stage, Stage::FigureArrives | Stage::Shatter | Stage::Ash) {
            paint_figure(&mut pix, w, ph, self.figure_x, tower, self.time, self.stage);
        }

        for p in &self.particles {
            paint_particle(&mut pix, w, ph, p);
        }

        if self.stage == Stage::Shatter {
            paint_shatter_flash(&mut pix, w, ph, tower, self.stage_time);
        }

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Geometry helpers ──────────────────────────────────────────────────────────

fn tower_rect(w: f64, ph: f64) -> (f64, f64, f64, f64) {
    let tw = (w * 0.22).clamp(16.0, 34.0);
    let th = (ph * 0.68).clamp(38.0, ph * 0.78);
    let x = w * 0.50 - tw * 0.5;
    let y = ph - th - 3.0;
    (x, y, tw, th)
}

fn in_bounds(w: usize, ph: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < ph as i32
}

// ── Paint helpers ─────────────────────────────────────────────────────────────

fn paint_background(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;

        let top = (4, 5, 20);
        let mid = (12, 10, 32);
        let bottom = (22, 20, 28);

        let mut col = if yf < 0.62 {
            lerp(top, mid, yf / 0.62)
        } else {
            lerp(mid, bottom, (yf - 0.62) / 0.38)
        };

        let haze = ((time * 0.12 + yf * 7.0).sin() + 1.0) * 0.5;
        col = blend(col, (35, 18, 55), haze * 0.035);

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    // Tiny stars.
    for i in 0..260usize {
        let sx = (i.wrapping_mul(2654435761) >> 4) % w.max(1);
        let sy = (i.wrapping_mul(2246822519) >> 4) % (ph / 2).max(1);
        let twinkle = ((time * 1.6 + i as f64 * 0.47).sin() + 1.0) * 0.5;
        if twinkle > 0.78 {
            let b = (120.0 + twinkle * 110.0) as u8;
            pix[sy][sx] = blend(pix[sy][sx], (b, b, 230), 0.55);
        }
    }

    // Ground.
    let ground = ph.saturating_sub(3);
    for y in ground..ph {
        for x in 0..w {
            pix[y][x] = blend(pix[y][x], (20, 32, 28), 0.85);
        }
    }
}

fn paint_moon(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    let x = w as f64 * 0.78;
    let y = ph as f64 * 0.16;
    let pulse = ((time * 0.25).sin() + 1.0) * 0.5;

    paint_soft_circle(pix, w, ph, x, y, 8.0, (225, 225, 205), 0.88);
    paint_soft_circle(pix, w, ph, x, y, 22.0, (130, 140, 175), 0.06 + pulse * 0.04);
}

fn paint_tower(pix: &mut Pix, w: usize, ph: usize, tower: (f64, f64, f64, f64), time: f64) {
    let (tx, ty, tw, th) = tower;

    let left = tx.floor() as i32;
    let right = (tx + tw).ceil() as i32;
    let top = ty.floor() as i32;
    let bottom = (ty + th).ceil() as i32;

    for y in top..=bottom {
        for x in left..=right {
            if !in_bounds(w, ph, x, y) {
                continue;
            }

            let xf = (x as f64 - tx) / tw.max(1.0);
            let yf = (y as f64 - ty) / th.max(1.0);

            let edge = xf < 0.08 || xf > 0.92;
            let brick = ((x / 4 + y / 3) % 2 == 0) as i32;
            let mortar = y % 5 == 0 || (x + brick * 2) % 9 == 0;

            let mut col = lerp((50, 52, 62), (78, 74, 80), 1.0 - yf);
            if edge {
                col = darken(col, 0.24);
            }
            if mortar {
                col = darken(col, 0.16);
            }

            pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, 0.96);
        }
    }

    // Battlements.
    for i in 0..5 {
        let bx = left + i * ((right - left).max(1) / 5);
        for y in top - 5..top {
            for x in bx..bx + 3 {
                if in_bounds(w, ph, x, y) {
                    pix[y as usize][x as usize] =
                        blend(pix[y as usize][x as usize], (68, 68, 78), 0.95);
                }
            }
        }
    }

    // Window slits.
    for row in 0..5 {
        let wy = top + 10 + row * ((bottom - top).max(1) / 6);
        for side in [0.32, 0.68] {
            let wx = (tx + tw * side) as i32;
            for dy in 0..5 {
                for dx in -1..=1 {
                    if in_bounds(w, ph, wx + dx, wy + dy) {
                        let glow = ((time * 0.9 + row as f64).sin() + 1.0) * 0.5;
                        let col = blend((8, 8, 18), (95, 130, 170), glow * 0.22);
                        pix[(wy + dy) as usize][(wx + dx) as usize] = col;
                    }
                }
            }
        }
    }
}

fn paint_seed(
    pix: &mut Pix,
    w: usize,
    ph: usize,
    tower: (f64, f64, f64, f64),
    stage: Stage,
    stage_time: f64,
    phase: f64,
) {
    if !matches!(stage, Stage::Seed | Stage::Growing) {
        return;
    }

    let (tx, ty, tw, th) = tower;
    let x = tx + tw * 0.50;
    let y = ty + th - 2.0;
    let pulse = ((stage_time * 5.0 + phase).sin() + 1.0) * 0.5;
    let r = 2.2 + pulse * 1.2;

    paint_soft_circle(pix, w, ph, x, y, r + 3.0, (100, 255, 220), 0.08 + pulse * 0.08);
    paint_soft_circle(pix, w, ph, x, y, r, (170, 255, 230), 0.85);
}

fn paint_vine(pix: &mut Pix, w: usize, ph: usize, nodes: &[VineNode], amount: f64, stage: Stage, time: f64) {
    let visible_count = ((nodes.len() as f64) * amount).ceil() as usize;
    let visible_count = visible_count.min(nodes.len());

    // Draw connected glowing vine.
    for pair in nodes[..visible_count].windows(2) {
        let a = pair[0];
        let b = pair[1];

        let growth_glow = if stage == Stage::Growing {
            (amount - a.t).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let pulse = ((time * 2.5 + a.phase).sin() + 1.0) * 0.5;
        let col = lerp((70, 220, 165), (175, 110, 255), pulse * 0.55);
        draw_soft_line(pix, w, ph, a.x, a.y, b.x, b.y, 1.6, col, 0.50 + growth_glow * 0.28);
        draw_soft_line(pix, w, ph, a.x, a.y, b.x, b.y, 4.5, col, 0.055);
    }

    // Crystal blooms.
    if matches!(stage, Stage::Bloom | Stage::FigureArrives) {
        for node in nodes[..visible_count].iter().filter(|n| n.bloom) {
            let pulse = ((time * 3.8 + node.phase).sin() + 1.0) * 0.5;
            let col = lerp((130, 255, 225), (245, 130, 255), pulse);

            for i in 0..5 {
                let a = TAU * i as f64 / 5.0 + node.phase * 0.25;
                paint_soft_circle(
                    pix,
                    w,
                    ph,
                    node.x + a.cos() * 2.2,
                    node.y + a.sin() * 1.5,
                    1.5,
                    col,
                    0.65,
                );
            }

            paint_soft_circle(pix, w, ph, node.x, node.y, 1.8, (245, 245, 220), 0.58);
            paint_soft_circle(pix, w, ph, node.x, node.y, 6.0, col, 0.06 + pulse * 0.06);
        }
    }
}

fn paint_figure(pix: &mut Pix, w: usize, ph: usize, figure_x: f64, tower: (f64, f64, f64, f64), time: f64, stage: Stage) {
    let (_, ty, _, th) = tower;
    let ground = ty + th;
    let x = figure_x;
    let y = ground - 1.0;

    let body = (35, 32, 30);
    paint_soft_circle(pix, w, ph, x, y - 5.0, 1.5, (210, 180, 135), 0.85);
    paint_rect(pix, w, ph, x - 1.5, y - 4.0, 3.0, 4.0, body, 0.92);

    // Legs.
    paint_rect(pix, w, ph, x - 1.8, y - 0.5, 1.0, 2.5, body, 0.85);
    paint_rect(pix, w, ph, x + 0.8, y - 0.5, 1.0, 2.5, body, 0.85);

    // Torch.
    let torch_x = x + 4.0;
    let torch_y = y - 5.0;
    draw_soft_line(pix, w, ph, x + 1.5, y - 3.0, torch_x, torch_y, 0.8, (95, 65, 35), 0.9);

    let flame = if matches!(stage, Stage::FigureArrives | Stage::Shatter) {
        1.0
    } else {
        0.4
    };
    let pulse = ((time * 9.0).sin() + 1.0) * 0.5;
    paint_soft_circle(pix, w, ph, torch_x, torch_y, 2.4, (255, 150, 45), flame * 0.65);
    paint_soft_circle(pix, w, ph, torch_x, torch_y - 0.8, 1.3, (255, 240, 130), flame * (0.45 + pulse * 0.25));
}

fn paint_particle(pix: &mut Pix, w: usize, ph: usize, p: &Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);

    match p.kind {
        ParticleKind::Spark => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.6, p.color, fade * 0.78);
        }
        ParticleKind::Crystal => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.4, p.color, fade * 0.72);
            paint_soft_circle(pix, w, ph, p.x, p.y, 3.5, p.color, fade * 0.08);
        }
        ParticleKind::Smoke => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 4.0 * (1.0 - fade + 0.35), p.color, fade * 0.22);
        }
        ParticleKind::Ash => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.1, p.color, fade * 0.32);
        }
    }
}

fn paint_shatter_flash(pix: &mut Pix, w: usize, ph: usize, tower: (f64, f64, f64, f64), stage_time: f64) {
    let alpha = (1.0 - stage_time / 0.55).clamp(0.0, 1.0);
    if alpha <= 0.0 {
        return;
    }

    let (tx, ty, tw, th) = tower;
    let x = tx + tw * 0.35;
    let y = ty + th * 0.60;
    paint_soft_circle(pix, w, ph, x, y, 24.0, (255, 230, 170), alpha * 0.35);
}

// ── Primitive drawing ─────────────────────────────────────────────────────────

fn draw_soft_line(
    pix: &mut Pix,
    w: usize,
    ph: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    radius: f64,
    col: Rgb,
    power: f64,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let steps = (dx.abs().max(dy.abs()) as usize + 1).max(1);

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = x1 + dx * t;
        let y = y1 + dy * t;
        paint_soft_circle(pix, w, ph, x, y, radius, col, power);
    }
}

fn paint_rect(pix: &mut Pix, w: usize, ph: usize, x: f64, y: f64, rw: f64, rh: f64, col: Rgb, power: f64) {
    let min_x = x.floor() as i32;
    let max_x = (x + rw).ceil() as i32;
    let min_y = y.floor() as i32;
    let max_y = (y + rh).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if in_bounds(w, ph, px, py) {
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, power);
            }
        }
    }
}

fn paint_soft_circle(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, power: f64) {
    let min_x = (cx - r - 1.0).floor() as i32;
    let max_x = (cx + r + 1.0).ceil() as i32;
    let min_y = (cy - r - 1.0).floor() as i32;
    let max_y = (cy + r + 1.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let dx = px as f64 - cx;
            let dy = py as f64 - cy;
            let d = (dx * dx + dy * dy).sqrt();

            if d <= r {
                let a = (1.0 - d / r.max(1.0)).powf(1.45) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn lerp(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |av: u8, bv: u8| (av as f64 + (bv as f64 - av as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend(bg: Rgb, fg: Rgb, alpha: f64) -> Rgb {
    let a = alpha.clamp(0.0, 1.0);
    let mix = |b: u8, f: u8| (b as f64 + (f as f64 - b as f64) * a) as u8;
    (mix(bg.0, fg.0), mix(bg.1, fg.1), mix(bg.2, fg.2))
}

fn darken(c: Rgb, f: f64) -> Rgb {
    let s = (1.0 - f).clamp(0.0, 1.0);
    (
        (c.0 as f64 * s) as u8,
        (c.1 as f64 * s) as u8,
        (c.2 as f64 * s) as u8,
    )
}

fn half_blocks(pix: &Pix, w: usize, h: usize, color_provider: &ColorProvider, t_abs: f64) -> String {
    let mut out = String::with_capacity(w * h * 24);

    let mut last_fg: Option<Rgb> = None;
    let mut last_bg: Option<Rgb> = None;

    for y in 0..h {
        let upper = y * 2;
        let lower = y * 2 + 1;

        for x in 0..w {
            let base_fg = pix[upper][x];
            let base_bg = if lower < pix.len() { pix[lower][x] } else { (0, 0, 0) };

            let fg = color_provider.tint(base_fg, t_abs, x as i32, upper as i32);
            let bg = color_provider.tint(base_bg, t_abs, x as i32, lower as i32);

            if Some(fg) != last_fg {
                out.push_str(&rgb(fg.0, fg.1, fg.2));
                last_fg = Some(fg);
            }

            if Some(bg) != last_bg {
                out.push_str(&bg_rgb(bg.0, bg.1, bg.2));
                last_bg = Some(bg);
            }

            out.push('▀');
        }

        out.push_str(RESET);
        last_fg = None;
        last_bg = None;

        if y < h - 1 {
            out.push('\n');
        }
    }

    out
}
