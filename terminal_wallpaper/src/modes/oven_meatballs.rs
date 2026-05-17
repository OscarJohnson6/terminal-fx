// ===== src/modes/oven_meatballs.rs =====
//
// OvenMeatballsMode
//
// A joke-mode turned real mode:
// You thought Metaballs was "Meatballs", so this mode is literally an oven
// cooking meatballs and other wheat-free foods.
//
// Scene loop:
//   1. Oven door opens.
//   2. Tray slides in with a random wheat-free food.
//   3. Food warms/cooks while glowing, steaming, sizzling.
//   4. Door opens again.
//   5. Tray slides out.
//   6. New food gets selected.
//
// Foods intentionally avoid wheat/bread/pasta:
//   meatballs, salmon, steak bites, roasted potatoes, peppers, mushrooms,
//   eggs, chicken skewers.

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    DoorOpeningIn,
    TrayIn,
    Cooking,
    DoorOpeningOut,
    TrayOut,
}

#[derive(Clone, Copy)]
enum FoodKind {
    Meatballs,
    Salmon,
    SteakBites,
    Potatoes,
    Peppers,
    Mushrooms,
    Eggs,
    ChickenSkewers,
}

impl FoodKind {
    fn name(self) -> &'static str {
        match self {
            FoodKind::Meatballs => "MEATBALLS",
            FoodKind::Salmon => "SALMON",
            FoodKind::SteakBites => "STEAK",
            FoodKind::Potatoes => "POTATOES",
            FoodKind::Peppers => "PEPPERS",
            FoodKind::Mushrooms => "MUSHROOMS",
            FoodKind::Eggs => "EGGS",
            FoodKind::ChickenSkewers => "CHICKEN",
        }
    }

    fn base_color(self) -> Rgb {
        match self {
            FoodKind::Meatballs => (135, 70, 45),
            FoodKind::Salmon => (230, 105, 85),
            FoodKind::SteakBites => (120, 55, 42),
            FoodKind::Potatoes => (205, 160, 85),
            FoodKind::Peppers => (215, 50, 45),
            FoodKind::Mushrooms => (165, 130, 95),
            FoodKind::Eggs => (245, 235, 190),
            FoodKind::ChickenSkewers => (210, 145, 80),
        }
    }
}

#[derive(Clone, Copy)]
struct FoodPiece {
    x: f64,
    y: f64,
    rx: f64,
    ry: f64,
    phase: f64,
    shade: f64,
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

#[derive(Clone, Copy)]
enum ParticleKind {
    Steam,
    Spark,
    Sizzle,
}

pub struct OvenMeatballsMode {
    speed: f64,
    color_provider: ColorProvider,
    time: f64,
    stage_time: f64,
    stage: Stage,
    food: FoodKind,
    food_pieces: Vec<FoodPiece>,
    particles: Vec<Particle>,
    tray_x: f64,
    door_open: f64,
    batch_crisp: f64,
    batch_burned: bool,
    last_dims: Option<(u16, u16)>,
}

impl OvenMeatballsMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            stage_time: 0.0,
            stage: Stage::DoorOpeningIn,
            food: FoodKind::Meatballs,
            food_pieces: Vec::new(),
            particles: Vec::new(),
            tray_x: -120.0,
            door_open: 0.0,
            batch_crisp: 1.0,
            batch_burned: false,
            last_dims: None,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();

        self.food = match rng.random_range(0..8) {
            0 => FoodKind::Meatballs,
            1 => FoodKind::Salmon,
            2 => FoodKind::SteakBites,
            3 => FoodKind::Potatoes,
            4 => FoodKind::Peppers,
            5 => FoodKind::Mushrooms,
            6 => FoodKind::Eggs,
            _ => FoodKind::ChickenSkewers,
        };

        self.food_pieces.clear();
        self.particles.clear();

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;
        let tray = tray_rect(w, ph, 0.0);

        let piece_count = match self.food {
            FoodKind::Meatballs => 13,
            FoodKind::Salmon => 4,
            FoodKind::SteakBites => 12,
            FoodKind::Potatoes => 16,
            FoodKind::Peppers => 18,
            FoodKind::Mushrooms => 14,
            FoodKind::Eggs => 6,
            FoodKind::ChickenSkewers => 10,
        };

        for i in 0..piece_count {
            let lane = (i as f64 / piece_count.max(1) as f64 - 0.5) * tray.2 * 0.75;
            let scatter_x = rng.random_range(-tray.2 * 0.12..tray.2 * 0.12);
            let scatter_y = rng.random_range(-tray.3 * 0.28..tray.3 * 0.28);

            let (rx, ry) = match self.food {
                FoodKind::Meatballs => (rng.random_range(2.6..4.3), rng.random_range(2.1..3.3)),
                FoodKind::Salmon => (rng.random_range(6.5..10.0), rng.random_range(2.3..3.4)),
                FoodKind::SteakBites => (rng.random_range(3.2..5.0), rng.random_range(2.0..3.1)),
                FoodKind::Potatoes => (rng.random_range(3.0..4.8), rng.random_range(2.0..3.0)),
                FoodKind::Peppers => (rng.random_range(4.0..6.5), rng.random_range(1.2..2.2)),
                FoodKind::Mushrooms => (rng.random_range(3.2..5.4), rng.random_range(2.0..3.1)),
                FoodKind::Eggs => (rng.random_range(5.0..7.2), rng.random_range(3.2..4.6)),
                FoodKind::ChickenSkewers => (rng.random_range(4.0..6.0), rng.random_range(2.0..3.0)),
            };

            self.food_pieces.push(FoodPiece {
                x: tray.0 + tray.2 * 0.5 + lane + scatter_x,
                y: tray.1 + tray.3 * 0.48 + scatter_y,
                rx,
                ry,
                phase: rng.random_range(0.0..TAU),
                shade: rng.random_range(0.78..1.18),
            });
        }

        self.tray_x = -w * 0.9;
        self.door_open = 0.0;

        // Most batches come out pleasantly crispy. Rarely, the oven runs too hot
        // and the food gets visibly overdone/burned.
        self.batch_burned = rng.random_range(0.0..1.0) < 0.13;
        self.batch_crisp = if self.batch_burned {
            rng.random_range(1.55..2.25)
        } else {
            rng.random_range(0.95..1.38)
        };

        self.stage = Stage::DoorOpeningIn;
        self.stage_time = 0.0;
        self.last_dims = Some((width, height));
    }

    fn next_stage(&mut self, width: u16, height: u16) {
        self.stage_time = 0.0;

        self.stage = match self.stage {
            Stage::DoorOpeningIn => Stage::TrayIn,
            Stage::TrayIn => Stage::Cooking,
            Stage::Cooking => Stage::DoorOpeningOut,
            Stage::DoorOpeningOut => Stage::TrayOut,
            Stage::TrayOut => {
                self.reset_scene(width, height);
                Stage::DoorOpeningIn
            }
        };
    }

    fn spawn_particle(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;
        let tray = tray_rect(w, ph, self.tray_x);
        let base_x = tray.0 + rng.random_range(tray.2 * 0.15..tray.2 * 0.85);
        let base_y = tray.1 + rng.random_range(tray.3 * 0.12..tray.3 * 0.65);

        let roll = rng.random_range(0.0..1.0);

        let steam_cutoff = if self.batch_burned { 0.50 } else { 0.68 };
        let spark_cutoff = if self.batch_burned { 0.92 } else { 0.88 };

        if roll < steam_cutoff {
            self.particles.push(Particle {
                x: base_x,
                y: base_y,
                vx: rng.random_range(-1.2..1.2),
                vy: rng.random_range(-8.0..-2.0),
                life: rng.random_range(0.8..2.2),
                max_life: 2.2,
                color: (190, 180, 170),
                kind: ParticleKind::Steam,
            });
        } else if roll < spark_cutoff {
            self.particles.push(Particle {
                x: base_x,
                y: base_y,
                vx: rng.random_range(-5.0..5.0),
                vy: rng.random_range(-6.0..-1.0),
                life: rng.random_range(0.25..0.65),
                max_life: 0.65,
                color: if self.batch_burned { (255, 115, 45) } else { (255, 190, 70) },
                kind: ParticleKind::Spark,
            });
        } else {
            self.particles.push(Particle {
                x: base_x,
                y: base_y,
                vx: rng.random_range(-2.5..2.5),
                vy: rng.random_range(-2.0..1.5),
                life: rng.random_range(0.20..0.55),
                max_life: 0.55,
                color: (255, 235, 160),
                kind: ParticleKind::Sizzle,
            });
        }
    }
}

impl Mode for OvenMeatballsMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.food_pieces.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = dt * self.speed;
        self.time += dt;
        self.stage_time += dt;

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        match self.stage {
            Stage::DoorOpeningIn => {
                self.door_open = ease_out((self.stage_time / 1.2).clamp(0.0, 1.0));
                self.tray_x = -w * 0.9;
                if self.stage_time > 1.2 {
                    self.next_stage(width, height);
                }
            }
            Stage::TrayIn => {
                self.door_open = 1.0;
                let p = ease_out((self.stage_time / 2.1).clamp(0.0, 1.0));
                self.tray_x = lerp_f(-w * 0.85, 0.0, p);

                if self.stage_time > 2.1 {
                    self.next_stage(width, height);
                }
            }
            Stage::Cooking => {
                self.door_open = (1.0 - ease_in_out((self.stage_time / 1.4).clamp(0.0, 1.0))).max(0.0);
                self.tray_x = 0.0;

                let mut rng = rand::rng();
                let cook_rate = if self.stage_time < 1.0 {
                    0.30
                } else if self.batch_burned && self.stage_time > 7.5 {
                    1.15
                } else {
                    0.78
                };

                for _ in 0..4 {
                    if rng.random_range(0.0..1.0) < cook_rate {
                        self.spawn_particle(width, height);
                    }
                }

                if self.stage_time > 12.5 {
                    self.next_stage(width, height);
                }
            }
            Stage::DoorOpeningOut => {
                self.door_open = ease_out((self.stage_time / 1.2).clamp(0.0, 1.0));
                self.tray_x = 0.0;
                if self.stage_time > 1.2 {
                    self.next_stage(width, height);
                }
            }
            Stage::TrayOut => {
                self.door_open = 1.0;
                let p = ease_in((self.stage_time / 2.0).clamp(0.0, 1.0));
                self.tray_x = lerp_f(0.0, w * 1.05, p);

                if self.stage_time > 2.0 {
                    self.next_stage(width, height);
                }
            }
        }

        for particle in &mut self.particles {
            particle.x += particle.vx * dt;
            particle.y += particle.vy * dt;

            match particle.kind {
                ParticleKind::Steam => {
                    particle.vy -= 0.7 * dt;
                    particle.vx += (self.time * 1.3 + particle.y * 0.05).sin() * dt * 0.9;
                }
                ParticleKind::Spark => {
                    particle.vy += 12.0 * dt;
                    particle.vx *= 0.98;
                }
                ParticleKind::Sizzle => {
                    particle.vy += 4.0 * dt;
                    particle.vx *= 0.94;
                }
            }

            particle.life -= dt;
        }

        self.particles.retain(|p| p.life > 0.0 && p.x > -10.0 && p.x < w + 10.0 && p.y > -15.0 && p.y < ph + 10.0);

        if self.particles.len() > 360 {
            let drop = self.particles.len() - 360;
            self.particles.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_oven_background(&mut pix, w, ph, self.time);
        paint_heating_elements(&mut pix, w, ph, self.time, self.stage);
        paint_oven_rack(&mut pix, w, ph);

        let tray = tray_rect(w as f64, ph as f64, self.tray_x);
        paint_tray(&mut pix, w, ph, tray, self.time);

        paint_food(
            &mut pix,
            w,
            ph,
            tray,
            self.food,
            &self.food_pieces,
            self.stage,
            self.stage_time,
            self.time,
            self.batch_crisp,
            self.batch_burned,
        );

        for p in &self.particles {
            paint_particle(&mut pix, w, ph, p);
        }

        paint_oven_door(&mut pix, w, ph, self.door_open, self.time);
        paint_label(&mut pix, w, ph, self.food.name(), self.stage, self.stage_time);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

fn tray_rect(w: f64, ph: f64, offset_x: f64) -> (f64, f64, f64, f64) {
    let tw = (w * 0.64).clamp(32.0, w * 0.82);
    let th = (ph * 0.17).clamp(8.0, 17.0);
    let x = w * 0.5 - tw * 0.5 + offset_x;
    let y = ph * 0.60;
    (x, y, tw, th)
}

fn in_bounds(w: usize, ph: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < ph as i32
}

fn ease_out(t: f64) -> f64 {
    1.0 - (1.0 - t).powi(3)
}

fn ease_in(t: f64) -> f64 {
    t * t * t
}

fn ease_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn lerp_f(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

fn paint_oven_background(pix: &mut Pix, w: usize, ph: usize, time: f64) {
    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let top = (15, 10, 8);
        let mid = (42, 20, 12);
        let bottom = (20, 12, 10);

        let mut col = if yf < 0.55 {
            lerp(top, mid, yf / 0.55)
        } else {
            lerp(mid, bottom, (yf - 0.55) / 0.45)
        };

        let heat = ((time * 3.0 + y as f64 * 0.10).sin() + 1.0) * 0.5;
        col = blend(col, (95, 32, 12), heat * 0.07);

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    for y in 0..ph {
        for x in 0..w {
            let edge = (x as f64 / w.max(1) as f64 - 0.5).abs() * 2.0;
            let shade = edge.powf(2.4) * 0.45;
            pix[y][x] = darken(pix[y][x], shade);
        }
    }

    for y in (ph / 5)..(ph * 3 / 4) {
        for x in 0..w {
            let mortar = y % 8 == 0 || (x + (y / 8) * 5) % 18 == 0;
            if mortar {
                pix[y][x] = blend(pix[y][x], (80, 42, 30), 0.16);
            }
        }
    }
}

fn paint_heating_elements(pix: &mut Pix, w: usize, ph: usize, time: f64, stage: Stage) {
    let power = match stage {
        Stage::Cooking => 1.0,
        Stage::TrayIn | Stage::DoorOpeningOut => 0.55,
        _ => 0.25,
    };

    for row in [ph as f64 * 0.18, ph as f64 * 0.86] {
        let y = row as i32;
        for x in (w as f64 * 0.18) as i32..(w as f64 * 0.82) as i32 {
            let wave = ((x as f64 * 0.22 + time * 5.0).sin() + 1.0) * 0.5;
            let yy = y + (wave * 2.0) as i32 - 1;
            let col = lerp((130, 30, 10), (255, 90, 22), power * (0.65 + wave * 0.35));
            paint_soft_circle(pix, w, ph, x as f64, yy as f64, 1.8, col, power * 0.62);
            paint_soft_circle(pix, w, ph, x as f64, yy as f64, 5.0, col, power * 0.045);
        }
    }
}

fn paint_oven_rack(pix: &mut Pix, w: usize, ph: usize) {
    let y = (ph as f64 * 0.77) as i32;
    for x in (w as f64 * 0.12) as i32..(w as f64 * 0.88) as i32 {
        for dy in -1..=1 {
            if in_bounds(w, ph, x, y + dy) {
                pix[(y + dy) as usize][x as usize] = blend(pix[(y + dy) as usize][x as usize], (95, 88, 82), 0.52);
            }
        }
    }

    for x in ((w as f64 * 0.14) as i32..(w as f64 * 0.88) as i32).step_by(6) {
        draw_soft_line(pix, w, ph, x as f64, y as f64 - 5.0, (x + 8) as f64, y as f64 + 4.0, 0.8, (75, 72, 70), 0.42);
    }
}

fn paint_tray(pix: &mut Pix, w: usize, ph: usize, tray: (f64, f64, f64, f64), time: f64) {
    let (x, y, tw, th) = tray;

    paint_rect(pix, w, ph, x, y, tw, th, (50, 48, 48), 0.84);
    paint_rect(pix, w, ph, x + 2.0, y + 2.0, tw - 4.0, th - 4.0, (72, 68, 62), 0.74);

    paint_rect(pix, w, ph, x, y, tw, 2.0, (140, 132, 120), 0.82);
    paint_rect(pix, w, ph, x, y + th - 2.0, tw, 2.0, (38, 36, 36), 0.78);
    paint_rect(pix, w, ph, x, y, 2.0, th, (125, 120, 112), 0.72);
    paint_rect(pix, w, ph, x + tw - 2.0, y, 2.0, th, (34, 32, 32), 0.72);

    let shimmer = ((time * 2.5).sin() + 1.0) * 0.5;
    paint_rect(pix, w, ph, x + 4.0, y + 3.0, tw * 0.40, 1.0, (180, 170, 150), 0.08 + shimmer * 0.08);
}

fn paint_food(
    pix: &mut Pix,
    w: usize,
    ph: usize,
    tray: (f64, f64, f64, f64),
    food: FoodKind,
    pieces: &[FoodPiece],
    stage: Stage,
    stage_time: f64,
    time: f64,
    batch_crisp: f64,
    batch_burned: bool,
) {
    let cook_raw = match stage {
        Stage::Cooking => (stage_time / 11.0).clamp(0.0, 1.0),
        Stage::DoorOpeningOut | Stage::TrayOut => 1.0,
        _ => 0.0,
    };

    let cook = (cook_raw * batch_crisp).clamp(0.0, 1.35);
    let char_level = (cook_raw * batch_crisp).clamp(0.0, 2.0);

    if matches!(food, FoodKind::ChickenSkewers) {
        for lane in -2..=2 {
            let yy = tray.1 + tray.3 * (0.30 + (lane + 2) as f64 * 0.10);
            draw_soft_line(
                pix,
                w,
                ph,
                tray.0 + tray.2 * 0.18,
                yy,
                tray.0 + tray.2 * 0.82,
                yy + 2.0,
                0.8,
                (145, 105, 62),
                0.75,
            );
        }
    }

    for piece in pieces {
        let wobble = (time * 1.8 + piece.phase).sin() * 0.35;
        let x = piece.x + tray.0 + tray.2 * 0.5 - (w as f64 * 0.5);
        let y = piece.y + wobble;
        let base = food.base_color();

        let browned = if batch_burned {
            lerp(base, (24, 18, 14), (cook * 0.72 * piece.shade).clamp(0.0, 1.0))
        } else {
            lerp(base, (74, 40, 25), (cook * 0.46 * piece.shade).clamp(0.0, 1.0))
        };
        let crisp_edge = if batch_burned { (20, 14, 10) } else { (92, 45, 22) };
        let highlight = lerp((255, 225, 165), (255, 145, 60), cook.clamp(0.0, 1.0));

        match food {
            FoodKind::Salmon => {
                paint_ellipse(pix, w, ph, x, y, piece.rx, piece.ry, browned, 0.90);
                draw_soft_line(pix, w, ph, x - piece.rx * 0.6, y, x + piece.rx * 0.65, y - 0.7, 0.6, (255, 185, 160), 0.35);
            }
            FoodKind::Peppers => {
                paint_ellipse(pix, w, ph, x, y, piece.rx, piece.ry, browned, 0.82);
                paint_soft_circle(pix, w, ph, x - piece.rx * 0.35, y - 0.25, 1.1, (255, 190, 70), 0.22);
            }
            FoodKind::Eggs => {
                paint_ellipse(pix, w, ph, x, y, piece.rx, piece.ry, (245, 240, 210), 0.86);
                paint_soft_circle(pix, w, ph, x + 0.3, y, piece.ry * 0.58, (245, 185, 60), 0.85);
                if char_level > 0.45 {
                    paint_soft_circle(pix, w, ph, x - 1.2, y - 0.6, 0.8, (255, 255, 230), 0.28);
                }
            }
            FoodKind::ChickenSkewers => {
                paint_ellipse(pix, w, ph, x, y, piece.rx, piece.ry, browned, 0.88);
                if char_level > 0.35 {
                    paint_soft_circle(pix, w, ph, x - 0.6, y - 0.5, 1.0, (95, 40, 20), 0.35);
                }
            }
            _ => {
                paint_ellipse(pix, w, ph, x, y, piece.rx, piece.ry, browned, 0.88);
                if char_level > 0.25 {
                    let char_x = x + piece.rx * 0.22;
                    let char_y = y - piece.ry * 0.20;
                    paint_soft_circle(pix, w, ph, char_x, char_y, 1.1, (70, 34, 20), char_level.min(1.0) * 0.45);
                }
            }
        }

        // Crisp/browned spots develop slowly. Burned batches get darker speckles
        // and a stronger charred edge.
        if char_level > 0.35 {
            let speckle_count = if batch_burned { 5 } else { 3 };
            for k in 0..speckle_count {
                let a = piece.phase + k as f64 * 2.17 + time * 0.03;
                let sx = x + a.cos() * piece.rx * 0.42;
                let sy = y + a.sin() * piece.ry * 0.38;
                let amount = ((char_level - 0.30) * 0.38).clamp(0.0, if batch_burned { 0.72 } else { 0.42 });
                paint_soft_circle(pix, w, ph, sx, sy, 0.7 + k as f64 * 0.08, crisp_edge, amount);
            }
        }

        if char_level > 0.70 {
            let edge_power = ((char_level - 0.70) * 0.20).clamp(0.0, if batch_burned { 0.32 } else { 0.14 });
            paint_ellipse(pix, w, ph, x, y, piece.rx + 0.6, piece.ry + 0.5, crisp_edge, edge_power);
        }

        if matches!(stage, Stage::Cooking) {
            let glow = (time * 6.0 + piece.phase).sin() * 0.5 + 0.5;
            paint_soft_circle(
                pix,
                w,
                ph,
                x,
                y,
                piece.rx.max(piece.ry) + 3.5,
                highlight,
                cook.clamp(0.0, 1.0) * glow * 0.026,
            );
        }
    }
}

fn paint_particle(pix: &mut Pix, w: usize, ph: usize, p: &Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);

    match p.kind {
        ParticleKind::Steam => {
            let r = 2.4 + (1.0 - fade) * 4.0;
            paint_soft_circle(pix, w, ph, p.x, p.y, r, p.color, fade * 0.15);
        }
        ParticleKind::Spark => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.3, p.color, fade * 0.72);
            paint_soft_circle(pix, w, ph, p.x, p.y, 3.6, p.color, fade * 0.08);
        }
        ParticleKind::Sizzle => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.0, p.color, fade * 0.55);
        }
    }
}

fn paint_oven_door(pix: &mut Pix, w: usize, ph: usize, door_open: f64, time: f64) {
    let alpha = (1.0 - door_open).clamp(0.0, 1.0);

    if alpha <= 0.02 {
        return;
    }

    for y in 0..ph {
        for x in 0..w {
            let edge = (x as f64 / w.max(1) as f64 - 0.5).abs() * 2.0;
            let glass = lerp((25, 18, 15), (75, 35, 18), (1.0 - edge).clamp(0.0, 1.0));
            pix[y][x] = blend(pix[y][x], glass, alpha * 0.32);
        }
    }

    let frame_col = (42, 38, 34);
    paint_rect(pix, w, ph, 0.0, 0.0, w as f64, 3.0, frame_col, alpha * 0.95);
    paint_rect(pix, w, ph, 0.0, ph as f64 - 4.0, w as f64, 4.0, frame_col, alpha * 0.95);
    paint_rect(pix, w, ph, 0.0, 0.0, 5.0, ph as f64, frame_col, alpha * 0.95);
    paint_rect(pix, w, ph, w as f64 - 5.0, 0.0, 5.0, ph as f64, frame_col, alpha * 0.95);

    let sweep = ((time * 0.8).sin() + 1.0) * 0.5;
    let x0 = w as f64 * (0.25 + sweep * 0.25);
    draw_soft_line(pix, w, ph, x0, ph as f64 * 0.12, x0 + w as f64 * 0.18, ph as f64 * 0.80, 1.2, (255, 210, 150), alpha * 0.14);
    draw_soft_line(pix, w, ph, x0 + 8.0, ph as f64 * 0.18, x0 + w as f64 * 0.20, ph as f64 * 0.72, 0.8, (255, 235, 190), alpha * 0.10);
}

fn paint_label(pix: &mut Pix, w: usize, ph: usize, label: &str, stage: Stage, stage_time: f64) {
    let panel_w = (label.len() as f64 * 4.0 + 18.0).clamp(38.0, w as f64 * 0.45);
    let x = w as f64 * 0.5 - panel_w * 0.5;
    let y = ph as f64 * 0.035;

    let heat = if matches!(stage, Stage::Cooking) {
        ((stage_time * 3.0).sin() + 1.0) * 0.5
    } else {
        0.2
    };

    paint_rect(pix, w, ph, x, y, panel_w, 7.0, (20, 18, 16), 0.82);
    paint_rect(pix, w, ph, x + 1.0, y + 1.0, panel_w - 2.0, 5.0, (60, 26, 14), 0.45 + heat * 0.22);

    for i in 0..label.len().min(12) {
        let px = x + 8.0 + i as f64 * 3.0;
        let py = y + 3.5;
        let blink = ((i as f64 * 0.7 + stage_time * 2.0).sin() + 1.0) * 0.5;
        paint_soft_circle(pix, w, ph, px, py, 0.9, (255, 150, 45), 0.45 + blink * 0.35);
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

fn paint_ellipse(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, rx: f64, ry: f64, col: Rgb, power: f64) {
    let min_x = (cx - rx - 1.0).floor() as i32;
    let max_x = (cx + rx + 1.0).ceil() as i32;
    let min_y = (cy - ry - 1.0).floor() as i32;
    let max_y = (cy + ry + 1.0).ceil() as i32;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            if !in_bounds(w, ph, px, py) {
                continue;
            }

            let dx = (px as f64 - cx) / rx.max(1.0);
            let dy = (py as f64 - cy) / ry.max(1.0);
            let d = dx * dx + dy * dy;

            if d <= 1.0 {
                let shade = 1.0 - d.sqrt();
                let lit = blend(col, (255, 230, 170), shade * 0.28);
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], lit, power);
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


fn oven_theme_tint(color_provider: &ColorProvider, base: Rgb, t: f64, x: i32, y: i32) -> Rgb {
    // This mode has strong intentional art direction: warm oven light, food color,
    // steam, and metal. A full rainbow tint makes it look like a disco filter.
    // So themes are deliberately subtle here.
    match color_provider.mode {
        crate::color::ColorMode::Rainbow => {
            let warmth = ((t * 0.8 + (x + y) as f64 * 0.025).sin() + 1.0) * 0.5;
            blend(base, (255, 125, 55), 0.035 + warmth * 0.025)
        }
        crate::color::ColorMode::Ocean => {
            blend(base, (60, 115, 125), 0.10)
        }
        crate::color::ColorMode::Sunset => {
            blend(base, (255, 120, 45), 0.13)
        }
        crate::color::ColorMode::Matrix => {
            blend(base, (45, 180, 70), 0.16)
        }
    }
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

            let fg = oven_theme_tint(color_provider, base_fg, t_abs, x as i32, upper as i32);
            let bg = oven_theme_tint(color_provider, base_bg, t_abs, x as i32, lower as i32);

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
