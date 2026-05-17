// ===== src/modes/potion_lab.rs =====
//
// PotionLabMode
//
// A cozy / chaotic alchemy lab wallpaper.
// Ingredients drop into a cauldron, the potion reacts, smoke rises,
// and random magical results appear before the lab settles and repeats.
//
// Visual loop:
//   1. Idle lab: bottles glow, cauldron simmers.
//   2. Ingredients fall into the cauldron.
//   3. Reaction builds: bubbles, smoke, sparks.
//   4. Result appears: ghost, slime, crystal, firework, tiny familiar, portal.
//   5. Cooldown, then next potion.
//
// Theme behavior:
//   Ocean  -> frost / blue potion lab
//   Sunset -> fire / orange potion lab
//   Matrix -> toxic green lab
//   Default/Rainbow -> purple arcane lab
//
// Suggested registry:
//
// In src/modes/mod.rs:
//   pub mod potion_lab;
//
// In mode_registry.rs descriptor imports:
//   potion_lab::PotionLabMode,
//
// Descriptor:
//   impl ModeDescriptor for PotionLabMode {
//       const ID:   &'static str = "potion_lab";
//       const NAME: &'static str = "Potion Lab";
//       const DESC: &'static str = "Bubbling cauldron experiments";
//       const FPS:  u32          = 50;
//   }
//
// In register_all!:
//   potion_lab::PotionLabMode,

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::{ColorMode, ColorProvider};
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LabPhase {
    Idle,
    Dropping,
    Brewing,
    Result,
    Cooldown,
}

#[derive(Clone, Copy)]
enum IngredientKind {
    Crystal,
    Leaf,
    Mushroom,
    StarDust,
    Eyeball,
    Feather,
    Flame,
    MoonDrop,
}

#[derive(Clone, Copy)]
struct Ingredient {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    spin: f64,
    kind: IngredientKind,
    color: Rgb,
    active: bool,
}

#[derive(Clone, Copy)]
enum ResultKind {
    Ghost,
    Slime,
    CrystalBloom,
    Firework,
    Familiar,
    Portal,
}

#[derive(Clone, Copy)]
enum ParticleKind {
    Bubble,
    Smoke,
    Spark,
    Magic,
    Splash,
    Star,
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
    phase: f64,
}

#[derive(Clone, Copy)]
struct Bottle {
    x: f64,
    y: f64,
    h: f64,
    color: Rgb,
    glow: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct LabGeom {
    cauldron_x: f64,
    cauldron_y: f64,
    cauldron_w: f64,
    cauldron_h: f64,
    table_y: f64,
}

pub struct PotionLabMode {
    speed: f64,
    color_provider: ColorProvider,

    time: f64,
    phase_time: f64,
    phase: LabPhase,

    result: ResultKind,
    ingredients: Vec<Ingredient>,
    particles: Vec<Particle>,
    bottles: Vec<Bottle>,

    reaction_power: f64,
    result_seed: f64,
    last_dims: Option<(u16, u16)>,
}

impl PotionLabMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            phase_time: 0.0,
            phase: LabPhase::Idle,
            result: ResultKind::Ghost,
            ingredients: Vec::new(),
            particles: Vec::new(),
            bottles: Vec::new(),
            reaction_power: 0.0,
            result_seed: 0.0,
            last_dims: None,
        }
    }

    fn reset_lab(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let geom = lab_geom(width, height);

        self.time = 0.0;
        self.phase_time = 0.0;
        self.phase = LabPhase::Idle;
        self.result = random_result(&mut rng);
        self.result_seed = rng.random_range(0.0..9999.0);
        self.reaction_power = 0.0;

        self.ingredients.clear();
        self.particles.clear();
        self.bottles.clear();

        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        let bottle_count = if width < 80 { 7 } else { 11 };
        for i in 0..bottle_count {
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            let offset = rng.random_range(geom.cauldron_w * 0.65..w * 0.42) * side;
            let x = (geom.cauldron_x + offset).clamp(4.0, w - 5.0);
            let y = geom.table_y - rng.random_range(5.0..11.0);
            let palette = potion_palette(self.color_provider.mode);

            self.bottles.push(Bottle {
                x,
                y,
                h: rng.random_range(5.0..11.0),
                color: palette[rng.random_range(0..palette.len())],
                glow: rng.random_range(0.15..0.55),
                phase: rng.random_range(0.0..TAU),
            });
        }

        // A few ambient magic motes so it does not start dead.
        for _ in 0..18 {
            self.spawn_particle(
                ParticleKind::Magic,
                rng.random_range(0.0..w),
                rng.random_range(ph * 0.18..geom.table_y - 8.0),
                ambient_magic_color(self.color_provider.mode),
            );
        }

        self.last_dims = Some((width, height));
    }

    fn next_phase(&mut self, width: u16, height: u16) {
        self.phase_time = 0.0;

        self.phase = match self.phase {
            LabPhase::Idle => {
                self.prepare_ingredients(width, height);
                LabPhase::Dropping
            }
            LabPhase::Dropping => LabPhase::Brewing,
            LabPhase::Brewing => {
                let mut rng = rand::rng();
                self.result = random_result(&mut rng);
                self.result_seed = rng.random_range(0.0..9999.0);
                LabPhase::Result
            }
            LabPhase::Result => LabPhase::Cooldown,
            LabPhase::Cooldown => {
                self.ingredients.clear();
                self.particles.retain(|p| !matches!(p.kind, ParticleKind::Splash));
                LabPhase::Idle
            }
        };
    }

    fn prepare_ingredients(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let geom = lab_geom(width, height);
        let w = width.max(1) as f64;

        self.ingredients.clear();

        let count = rng.random_range(4..=7);
        for i in 0..count {
            let kind = random_ingredient(&mut rng);
            let color = ingredient_color(kind, self.color_provider.mode);
            let spread = (i as f64 / count as f64 - 0.5) * geom.cauldron_w * 1.25;

            self.ingredients.push(Ingredient {
                x: geom.cauldron_x + spread + rng.random_range(-4.0..4.0),
                y: rng.random_range(-18.0..-3.0) - i as f64 * 2.0,
                vx: rng.random_range(-3.0..3.0),
                vy: rng.random_range(16.0..27.0),
                spin: rng.random_range(0.0..TAU),
                kind,
                color,
                active: true,
            });
        }

        // One dramatic side ingredient sometimes flies in like someone threw it.
        if rng.random_range(0.0..1.0) < 0.42 {
            let kind = random_ingredient(&mut rng);
            self.ingredients.push(Ingredient {
                x: if rng.random_bool(0.5) { -8.0 } else { w + 8.0 },
                y: rng.random_range(4.0..geom.cauldron_y - 10.0),
                vx: if rng.random_bool(0.5) {
                    rng.random_range(24.0..42.0)
                } else {
                    -rng.random_range(24.0..42.0)
                },
                vy: rng.random_range(3.0..11.0),
                spin: rng.random_range(0.0..TAU),
                kind,
                color: ingredient_color(kind, self.color_provider.mode),
                active: true,
            });
        }
    }

    fn spawn_particle(&mut self, kind: ParticleKind, x: f64, y: f64, color: Rgb) {
        if self.particles.len() > 900 {
            return;
        }

        let mut rng = rand::rng();

        let (vx, vy, life) = match kind {
            ParticleKind::Bubble => (
                rng.random_range(-2.5..2.5),
                rng.random_range(-10.0..-3.0),
                rng.random_range(0.8..2.0),
            ),
            ParticleKind::Smoke => (
                rng.random_range(-4.0..4.0),
                rng.random_range(-14.0..-4.0),
                rng.random_range(1.6..3.8),
            ),
            ParticleKind::Spark => (
                rng.random_range(-11.0..11.0),
                rng.random_range(-17.0..-4.0),
                rng.random_range(0.35..1.0),
            ),
            ParticleKind::Magic => (
                rng.random_range(-3.0..3.0),
                rng.random_range(-5.0..1.0),
                rng.random_range(1.2..3.5),
            ),
            ParticleKind::Splash => (
                rng.random_range(-9.0..9.0),
                rng.random_range(-15.0..-4.0),
                rng.random_range(0.45..1.2),
            ),
            ParticleKind::Star => (
                rng.random_range(-15.0..15.0),
                rng.random_range(-20.0..-6.0),
                rng.random_range(0.8..2.2),
            ),
        };

        self.particles.push(Particle {
            x,
            y,
            vx,
            vy,
            life,
            max_life: life,
            color,
            kind,
            phase: rng.random_range(0.0..TAU),
        });
    }

    fn spawn_reaction_particles(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let geom = lab_geom(width, height);
        let potion = potion_color(self.color_provider.mode);

        let rate = match self.phase {
            LabPhase::Idle => 2,
            LabPhase::Dropping => 5,
            LabPhase::Brewing => 13,
            LabPhase::Result => 10,
            LabPhase::Cooldown => 4,
        };

        for _ in 0..rate {
            let x = geom.cauldron_x + rng.random_range(-geom.cauldron_w * 0.34..geom.cauldron_w * 0.34);
            let y = geom.cauldron_y - geom.cauldron_h * 0.42 + rng.random_range(-1.0..2.0);

            let roll = rng.random_range(0.0..1.0);
            let kind = if roll < 0.42 {
                ParticleKind::Bubble
            } else if roll < 0.67 {
                ParticleKind::Smoke
            } else if roll < 0.87 {
                ParticleKind::Magic
            } else {
                ParticleKind::Spark
            };

            let c = match kind {
                ParticleKind::Smoke => smoke_color(self.color_provider.mode),
                ParticleKind::Spark | ParticleKind::Star => spark_color(self.color_provider.mode),
                _ => potion,
            };

            if rng.random_range(0.0..1.0) < self.reaction_power.max(0.12) {
                self.spawn_particle(kind, x, y, c);
            }
        }
    }
}

impl Mode for PotionLabMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.bottles.is_empty() {
            self.reset_lab(width, height);
        }

        let dt = (dt * self.speed).min(0.06);
        self.time += dt;
        self.phase_time += dt;

        match self.phase {
            LabPhase::Idle => {
                self.reaction_power += (0.15 - self.reaction_power) * dt * 1.2;
                if self.phase_time > 4.0 {
                    self.next_phase(width, height);
                }
            }
            LabPhase::Dropping => {
                self.reaction_power += (0.45 - self.reaction_power) * dt * 1.6;

                let geom = lab_geom(width, height);
                let mut splashes: Vec<(f64, f64, Rgb)> = Vec::new();

                for ing in &mut self.ingredients {
                    if !ing.active {
                        continue;
                    }

                    ing.vy += 18.0 * dt;
                    ing.x += ing.vx * dt;
                    ing.y += ing.vy * dt;
                    ing.spin += dt * 6.0;

                    let hit_y = geom.cauldron_y - geom.cauldron_h * 0.36;
                    if ing.y >= hit_y
                        && (ing.x - geom.cauldron_x).abs() < geom.cauldron_w * 0.45
                    {
                        ing.active = false;
                        splashes.push((ing.x, hit_y, ing.color));
                    }
                }

                for (x, y, c) in splashes {
                    for _ in 0..9 {
                        self.spawn_particle(ParticleKind::Splash, x, y, c);
                    }
                    for _ in 0..4 {
                        self.spawn_particle(ParticleKind::Spark, x, y, spark_color(self.color_provider.mode));
                    }
                }

                if self.phase_time > 4.2 || self.ingredients.iter().all(|i| !i.active) {
                    self.next_phase(width, height);
                }
            }
            LabPhase::Brewing => {
                let build = (self.phase_time / 6.0).clamp(0.0, 1.0);
                let wobble = ((self.time * 6.0).sin() + 1.0) * 0.5;
                self.reaction_power += ((0.52 + build * 0.52 + wobble * 0.12) - self.reaction_power) * dt * 2.0;

                self.spawn_reaction_particles(width, height);

                if self.phase_time > 8.0 {
                    self.next_phase(width, height);
                    let geom = lab_geom(width, height);
                    for _ in 0..42 {
                        self.spawn_particle(
                            ParticleKind::Star,
                            geom.cauldron_x,
                            geom.cauldron_y - geom.cauldron_h,
                            spark_color(self.color_provider.mode),
                        );
                    }
                }
            }
            LabPhase::Result => {
                self.reaction_power += (0.72 - self.reaction_power) * dt * 2.4;
                self.spawn_reaction_particles(width, height);

                if self.phase_time > 5.8 {
                    self.next_phase(width, height);
                }
            }
            LabPhase::Cooldown => {
                self.reaction_power += (0.12 - self.reaction_power) * dt * 1.4;

                if self.phase_time > 3.3 {
                    self.next_phase(width, height);
                }
            }
        }

        // Ambient bottle flicker.
        for bottle in &mut self.bottles {
            bottle.phase += dt * 2.0;
            bottle.glow += (((self.time + bottle.phase).sin() * 0.5 + 0.5) * 0.45 - bottle.glow) * dt * 0.8;
        }

        for p in &mut self.particles {
            p.phase += dt * 3.0;
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            match p.kind {
                ParticleKind::Bubble => {
                    p.vx += (self.time * 2.0 + p.phase).sin() * dt * 2.0;
                    p.vy -= 0.4 * dt;
                }
                ParticleKind::Smoke => {
                    p.vx += (self.time * 1.4 + p.y * 0.06 + p.phase).sin() * dt * 3.0;
                    p.vy -= 1.1 * dt;
                }
                ParticleKind::Spark | ParticleKind::Splash | ParticleKind::Star => {
                    p.vy += 16.0 * dt;
                    p.vx *= 0.97;
                }
                ParticleKind::Magic => {
                    p.vx += (self.time * 2.3 + p.phase).cos() * dt * 2.5;
                    p.vy += (self.time * 1.7 + p.phase).sin() * dt * 1.0;
                }
            }

            p.life -= dt;
        }

        self.particles.retain(|p| p.life > 0.0);

        if self.particles.len() > 900 {
            let drop = self.particles.len() - 900;
            self.particles.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;
        let ph = h * 2;
        let geom = lab_geom(width, height);

        let mut pix: Pix = vec![vec![(0, 0, 0); w]; ph];

        paint_lab_background(&mut pix, w, ph, self.color_provider.mode, self.time);
        paint_shelves(&mut pix, w, ph, self.color_provider.mode, self.time);

        for bottle in &self.bottles {
            paint_bottle(&mut pix, w, ph, *bottle, self.time);
        }

        paint_table(&mut pix, w, ph, &geom, self.color_provider.mode, self.time);
        paint_cauldron_glow(&mut pix, w, ph, &geom, self.color_provider.mode, self.reaction_power, self.time);
        paint_cauldron(&mut pix, w, ph, &geom, self.color_provider.mode, self.reaction_power, self.time);

        for ing in &self.ingredients {
            if ing.active {
                paint_ingredient(&mut pix, w, ph, *ing, self.time);
            }
        }

        for p in &self.particles {
            paint_particle(&mut pix, w, ph, *p);
        }

        if self.phase == LabPhase::Result {
            paint_result(&mut pix, w, ph, &geom, self.result, self.result_seed, self.phase_time, self.color_provider.mode);
        }

        paint_foreground_tools(&mut pix, w, ph, &geom, self.time);
        paint_phase_runes(&mut pix, w, ph, self.phase, self.phase_time, self.color_provider.mode);
        paint_vignette(&mut pix, w, ph);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Layout / random helpers ──────────────────────────────────────────────────

fn lab_geom(width: u16, height: u16) -> LabGeom {
    let w = width.max(1) as f64;
    let ph = height.max(1) as f64 * 2.0;
    let table_y = ph * 0.76;

    LabGeom {
        cauldron_x: w * 0.50,
        cauldron_y: table_y - 3.0,
        cauldron_w: (w * 0.30).clamp(22.0, 46.0),
        cauldron_h: (ph * 0.16).clamp(10.0, 18.0),
        table_y,
    }
}

fn random_ingredient(rng: &mut impl RngExt) -> IngredientKind {
    match rng.random_range(0..8) {
        0 => IngredientKind::Crystal,
        1 => IngredientKind::Leaf,
        2 => IngredientKind::Mushroom,
        3 => IngredientKind::StarDust,
        4 => IngredientKind::Eyeball,
        5 => IngredientKind::Feather,
        6 => IngredientKind::Flame,
        _ => IngredientKind::MoonDrop,
    }
}

fn random_result(rng: &mut impl RngExt) -> ResultKind {
    match rng.random_range(0..6) {
        0 => ResultKind::Ghost,
        1 => ResultKind::Slime,
        2 => ResultKind::CrystalBloom,
        3 => ResultKind::Firework,
        4 => ResultKind::Familiar,
        _ => ResultKind::Portal,
    }
}

// ── Painting ─────────────────────────────────────────────────────────────────

fn paint_lab_background(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64) {
    let (top, bottom) = match mode {
        ColorMode::Ocean => ((5, 18, 34), (18, 48, 66)),
        ColorMode::Sunset => ((38, 18, 24), (78, 42, 32)),
        ColorMode::Matrix => ((0, 12, 7), (8, 35, 18)),
        ColorMode::Rainbow => ((16, 12, 32), (46, 28, 62)),
    };

    for y in 0..ph {
        let t = y as f64 / ph.max(1) as f64;
        let mut col = lerp(top, bottom, t.powf(0.82));
        let wave = ((time * 0.25 + y as f64 * 0.05).sin() + 1.0) * 0.5;
        col = blend(col, potion_color(mode), wave * 0.018);

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    // Stone wall blocks.
    for y in (ph / 7..ph * 3 / 4).step_by(8) {
        paint_rect(pix, w, ph, 0.0, y as f64, w as f64, 1.0, (70, 64, 72), 0.10);
    }

    for y in (ph / 7..ph * 3 / 4).step_by(8) {
        let offset = if (y / 8) % 2 == 0 { 0 } else { 10 };
        for x in (offset..w).step_by(20) {
            paint_rect(pix, w, ph, x as f64, y as f64, 1.0, 8.0, (70, 64, 72), 0.08);
        }
    }
}

fn paint_shelves(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64) {
    let shelf_col = match mode {
        ColorMode::Ocean => (68, 52, 42),
        ColorMode::Sunset => (88, 48, 30),
        ColorMode::Matrix => (35, 58, 35),
        ColorMode::Rainbow => (66, 42, 58),
    };

    for y in [ph as f64 * 0.24, ph as f64 * 0.39] {
        paint_rect(pix, w, ph, w as f64 * 0.05, y, w as f64 * 0.90, 2.5, shelf_col, 0.74);
        paint_rect(pix, w, ph, w as f64 * 0.05, y + 2.0, w as f64 * 0.90, 1.0, (22, 17, 15), 0.52);
    }

    // Hanging herbs / crystals.
    for i in 0..9 {
        let x = w as f64 * (0.12 + i as f64 * 0.095);
        let y = ph as f64 * 0.14;
        let sway = (time * 0.9 + i as f64).sin() * 1.2;
        draw_soft_line(pix, w, ph, x, y, x + sway, y + 10.0, 0.45, (80, 70, 55), 0.55);

        let c = match i % 4 {
            0 => (95, 180, 90),
            1 => (180, 80, 210),
            2 => (80, 190, 230),
            _ => (230, 180, 85),
        };
        paint_soft_circle(pix, w, ph, x + sway, y + 11.0, 1.4, c, 0.45);
    }
}

fn paint_bottle(pix: &mut Pix, w: usize, ph: usize, bottle: Bottle, time: f64) {
    let pulse = ((time * 2.0 + bottle.phase).sin() + 1.0) * 0.5;
    let glow = bottle.glow * (0.55 + pulse * 0.55);

    let x = bottle.x;
    let y = bottle.y;
    let h = bottle.h;

    paint_soft_circle(pix, w, ph, x, y + h * 0.45, h * 0.70, bottle.color, glow * 0.07);
    paint_rect(pix, w, ph, x - 2.0, y - h, 4.0, h * 0.35, (165, 185, 190), 0.28);
    paint_rect(pix, w, ph, x - 3.0, y - h * 0.64, 6.0, h * 0.95, (185, 205, 210), 0.18);
    paint_rect(pix, w, ph, x - 2.0, y - h * 0.28, 4.0, h * 0.52, bottle.color, 0.42 + glow * 0.25);
    paint_soft_circle(pix, w, ph, x - 1.2, y - h * 0.18, 0.8, (255, 255, 255), 0.18);
}

fn paint_table(pix: &mut Pix, w: usize, ph: usize, geom: &LabGeom, mode: ColorMode, time: f64) {
    let wood = match mode {
        ColorMode::Ocean => (58, 44, 38),
        ColorMode::Sunset => (88, 48, 30),
        ColorMode::Matrix => (34, 48, 32),
        ColorMode::Rainbow => (70, 42, 50),
    };

    paint_rect(pix, w, ph, 0.0, geom.table_y, w as f64, ph as f64 - geom.table_y, wood, 0.88);
    paint_rect(pix, w, ph, 0.0, geom.table_y, w as f64, 3.0, (120, 82, 50), 0.56);

    for y in geom.table_y as usize..ph {
        let yy = y as f64;
        let wave = ((time * 0.3 + yy * 0.22).sin() + 1.0) * 0.5;
        for x in (0..w).step_by(17) {
            set_blend(pix, w, ph, x as i32, y as i32, (20, 14, 10), 0.08 + wave * 0.04);
        }
    }
}

fn paint_cauldron_glow(pix: &mut Pix, w: usize, ph: usize, geom: &LabGeom, mode: ColorMode, power: f64, time: f64) {
    let potion = potion_color(mode);
    let pulse = ((time * 4.0).sin() + 1.0) * 0.5;
    let p = power.clamp(0.0, 1.2);

    paint_soft_ellipse(
        pix,
        w,
        ph,
        geom.cauldron_x,
        geom.cauldron_y - geom.cauldron_h * 0.34,
        geom.cauldron_w * (0.52 + p * 0.28),
        geom.cauldron_h * (0.9 + p * 0.45),
        potion,
        0.09 + p * 0.13 + pulse * 0.035,
    );

    paint_soft_ellipse(
        pix,
        w,
        ph,
        geom.cauldron_x,
        geom.cauldron_y + geom.cauldron_h * 0.26,
        geom.cauldron_w * 0.70,
        geom.cauldron_h * 0.35,
        spark_color(mode),
        0.04 + p * 0.08,
    );
}

fn paint_cauldron(pix: &mut Pix, w: usize, ph: usize, geom: &LabGeom, mode: ColorMode, power: f64, time: f64) {
    let metal = match mode {
        ColorMode::Ocean => (42, 58, 66),
        ColorMode::Sunset => (65, 42, 36),
        ColorMode::Matrix => (26, 50, 34),
        ColorMode::Rainbow => (48, 40, 58),
    };

    let cx = geom.cauldron_x;
    let cy = geom.cauldron_y;
    let cw = geom.cauldron_w;
    let ch = geom.cauldron_h;

    // body
    paint_soft_ellipse(pix, w, ph, cx, cy, cw * 0.52, ch * 0.62, (0, 0, 0), 0.40);
    paint_ellipse(pix, w, ph, cx, cy, cw * 0.52, ch * 0.55, metal, 0.92);

    // opening/potion surface
    let potion = potion_color(mode);
    let surface_y = cy - ch * 0.35;
    paint_ellipse(pix, w, ph, cx, surface_y, cw * 0.45, ch * 0.20, (8, 7, 9), 0.94);
    let wave = ((time * 5.0).sin() + 1.0) * 0.5;
    paint_ellipse(
        pix,
        w,
        ph,
        cx,
        surface_y + wave * 0.8,
        cw * 0.39,
        ch * 0.14,
        potion,
        0.66 + power * 0.25,
    );

    // rim
    paint_ring_ellipse(pix, w, ph, cx, surface_y, cw * 0.48, ch * 0.21, (160, 150, 135), 0.60);

    // legs
    paint_rect(pix, w, ph, cx - cw * 0.38, cy + ch * 0.37, 3.0, ch * 0.28, darken(metal, 0.30), 0.82);
    paint_rect(pix, w, ph, cx + cw * 0.32, cy + ch * 0.37, 3.0, ch * 0.28, darken(metal, 0.30), 0.82);

    // fire
    for i in 0..7 {
        let x = cx - cw * 0.24 + i as f64 * cw * 0.08;
        let flame = ((time * 8.0 + i as f64).sin() + 1.0) * 0.5;
        paint_soft_ellipse(
            pix,
            w,
            ph,
            x,
            cy + ch * 0.63 - flame * 2.0,
            2.0,
            4.5 + flame * 2.0,
            spark_color(mode),
            0.22 + power * 0.17,
        );
    }
}

fn paint_ingredient(pix: &mut Pix, w: usize, ph: usize, ing: Ingredient, _time: f64) {
    let x = ing.x;
    let y = ing.y;
    let spin = ing.spin;

    match ing.kind {
        IngredientKind::Crystal => {
            paint_soft_circle(pix, w, ph, x, y, 3.0, ing.color, 0.16);
            draw_soft_line(pix, w, ph, x, y - 3.0, x + 2.0, y, 0.8, ing.color, 0.70);
            draw_soft_line(pix, w, ph, x + 2.0, y, x, y + 3.0, 0.8, ing.color, 0.70);
            draw_soft_line(pix, w, ph, x, y + 3.0, x - 2.0, y, 0.8, ing.color, 0.70);
            draw_soft_line(pix, w, ph, x - 2.0, y, x, y - 3.0, 0.8, ing.color, 0.70);
        }
        IngredientKind::Leaf | IngredientKind::Feather => {
            let dx = spin.cos() * 3.0;
            let dy = spin.sin() * 1.2;
            draw_soft_line(pix, w, ph, x - dx, y - dy, x + dx, y + dy, 1.0, ing.color, 0.78);
            paint_soft_circle(pix, w, ph, x, y, 1.0, (255, 255, 210), 0.18);
        }
        IngredientKind::Mushroom => {
            paint_rect(pix, w, ph, x - 0.8, y, 1.6, 3.0, (225, 205, 165), 0.72);
            paint_soft_ellipse(pix, w, ph, x, y, 3.2, 2.0, ing.color, 0.78);
        }
        IngredientKind::StarDust => {
            for i in 0..5 {
                let a = spin + i as f64 * TAU / 5.0;
                paint_soft_circle(pix, w, ph, x + a.cos() * 2.2, y + a.sin() * 2.2, 0.8, ing.color, 0.72);
            }
            paint_soft_circle(pix, w, ph, x, y, 4.2, ing.color, 0.06);
        }
        IngredientKind::Eyeball => {
            paint_soft_circle(pix, w, ph, x, y, 2.5, (235, 230, 205), 0.82);
            paint_soft_circle(pix, w, ph, x + spin.cos() * 0.8, y + spin.sin() * 0.5, 1.0, ing.color, 0.88);
            paint_soft_circle(pix, w, ph, x + spin.cos() * 1.0, y + spin.sin() * 0.5, 0.4, (10, 10, 12), 0.92);
        }
        IngredientKind::Flame => {
            paint_soft_ellipse(pix, w, ph, x, y, 2.3, 4.0, ing.color, 0.55);
            paint_soft_ellipse(pix, w, ph, x, y + 0.8, 1.3, 2.8, (255, 235, 120), 0.36);
        }
        IngredientKind::MoonDrop => {
            paint_soft_circle(pix, w, ph, x, y, 2.4, ing.color, 0.68);
            paint_soft_circle(pix, w, ph, x + 1.0, y - 0.5, 2.1, (8, 10, 20), 0.45);
        }
    }
}

fn paint_particle(pix: &mut Pix, w: usize, ph: usize, p: Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);

    match p.kind {
        ParticleKind::Bubble => {
            paint_ring(pix, w, ph, p.x, p.y, 1.4 + (1.0 - fade) * 1.4, p.color, fade * 0.38);
        }
        ParticleKind::Smoke => {
            let r = 2.0 + (1.0 - fade) * 7.5;
            paint_soft_circle(pix, w, ph, p.x, p.y, r, p.color, fade * 0.16);
        }
        ParticleKind::Spark | ParticleKind::Splash | ParticleKind::Star => {
            let r = if matches!(p.kind, ParticleKind::Star) { 1.6 } else { 1.1 };
            paint_soft_circle(pix, w, ph, p.x, p.y, r, p.color, fade * 0.65);
            paint_soft_circle(pix, w, ph, p.x, p.y, r * 3.2, p.color, fade * 0.06);
        }
        ParticleKind::Magic => {
            let pulse = ((p.phase + p.life * 4.0).sin() + 1.0) * 0.5;
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.0, p.color, fade * (0.30 + pulse * 0.35));
            paint_soft_circle(pix, w, ph, p.x, p.y, 4.5, p.color, fade * 0.045);
        }
    }
}

fn paint_result(pix: &mut Pix, w: usize, ph: usize, geom: &LabGeom, result: ResultKind, seed: f64, phase_time: f64, mode: ColorMode) {
    let cx = geom.cauldron_x;
    let base_y = geom.cauldron_y - geom.cauldron_h - 3.0;
    let rise = (phase_time / 2.0).clamp(0.0, 1.0);
    let y = base_y - rise * 10.0 + (phase_time * 2.0).sin() * 1.2;

    let main = potion_color(mode);
    let spark = spark_color(mode);

    match result {
        ResultKind::Ghost => {
            let alpha = (phase_time / 0.8).clamp(0.0, 1.0) * ((5.8 - phase_time) / 2.0).clamp(0.0, 1.0).min(1.0);
            paint_soft_ellipse(pix, w, ph, cx, y, 7.0, 8.5, (190, 220, 255), alpha * 0.34);
            paint_soft_circle(pix, w, ph, cx - 2.0, y - 2.0, 0.8, (10, 12, 18), alpha);
            paint_soft_circle(pix, w, ph, cx + 2.0, y - 2.0, 0.8, (10, 12, 18), alpha);
            for i in -2..=2 {
                paint_soft_circle(pix, w, ph, cx + i as f64 * 2.0, y + 6.0 + (phase_time * 5.0 + i as f64).sin(), 1.2, (190, 220, 255), alpha * 0.22);
            }
        }
        ResultKind::Slime => {
            let wobble = ((phase_time * 5.0).sin() + 1.0) * 0.5;
            paint_soft_ellipse(pix, w, ph, cx, y + 2.0, 9.0 + wobble * 2.0, 5.0, main, 0.58);
            paint_soft_circle(pix, w, ph, cx - 3.0, y, 0.9, (245, 255, 245), 0.85);
            paint_soft_circle(pix, w, ph, cx + 3.0, y, 0.9, (245, 255, 245), 0.85);
            paint_soft_circle(pix, w, ph, cx - 3.0, y, 0.4, (10, 12, 10), 0.9);
            paint_soft_circle(pix, w, ph, cx + 3.0, y, 0.4, (10, 12, 10), 0.9);
        }
        ResultKind::CrystalBloom => {
            for i in 0..9 {
                let a = seed + i as f64 * TAU / 9.0 + phase_time * 0.18;
                let len = 5.0 + (i % 3) as f64 * 3.0;
                draw_soft_line(
                    pix,
                    w,
                    ph,
                    cx,
                    y,
                    cx + a.cos() * len,
                    y + a.sin() * len,
                    1.0,
                    main,
                    0.72,
                );
            }
            paint_soft_circle(pix, w, ph, cx, y, 5.0, spark, 0.18);
        }
        ResultKind::Firework => {
            let r = (phase_time * 10.0).min(20.0);
            for i in 0..18 {
                let a = seed + i as f64 * TAU / 18.0;
                let px = cx + a.cos() * r;
                let py = y + a.sin() * r * 0.70;
                paint_soft_circle(pix, w, ph, px, py, 1.2, if i % 2 == 0 { main } else { spark }, 0.62);
                paint_soft_circle(pix, w, ph, px, py, 4.0, spark, 0.045);
            }
        }
        ResultKind::Familiar => {
            let bob = (phase_time * 4.0).sin();
            paint_soft_ellipse(pix, w, ph, cx, y + bob, 5.5, 4.0, (40, 30, 50), 0.82);
            paint_soft_circle(pix, w, ph, cx - 2.0, y - 1.0 + bob, 0.8, spark, 0.9);
            paint_soft_circle(pix, w, ph, cx + 2.0, y - 1.0 + bob, 0.8, spark, 0.9);
            draw_soft_line(pix, w, ph, cx - 4.0, y - 2.0 + bob, cx - 8.0, y - 6.0 + bob, 0.8, (45, 35, 55), 0.75);
            draw_soft_line(pix, w, ph, cx + 4.0, y - 2.0 + bob, cx + 8.0, y - 6.0 + bob, 0.8, (45, 35, 55), 0.75);
        }
        ResultKind::Portal => {
            for i in 0..5 {
                let r = 5.0 + i as f64 * 3.0;
                let rot = phase_time * (1.0 + i as f64 * 0.2);
                for k in 0..20 {
                    let a = rot + k as f64 * TAU / 20.0;
                    let px = cx + a.cos() * r;
                    let py = y + a.sin() * r * 0.55;
                    if k % 3 == 0 {
                        paint_soft_circle(pix, w, ph, px, py, 0.8, if i % 2 == 0 { main } else { spark }, 0.35);
                    }
                }
            }
            paint_soft_ellipse(pix, w, ph, cx, y, 12.0, 7.0, main, 0.12);
        }
    }
}

fn paint_foreground_tools(pix: &mut Pix, w: usize, ph: usize, geom: &LabGeom, time: f64) {
    // Spoon / wand.
    let x1 = geom.cauldron_x - geom.cauldron_w * 0.66;
    let y1 = geom.table_y - 2.0;
    let x2 = x1 + 16.0;
    let y2 = y1 - 6.0 + (time * 0.7).sin();
    draw_soft_line(pix, w, ph, x1, y1, x2, y2, 0.8, (150, 110, 70), 0.75);
    paint_soft_circle(pix, w, ph, x2, y2, 1.4, (255, 230, 150), 0.36);

    // Book.
    let bx = geom.cauldron_x + geom.cauldron_w * 0.58;
    let by = geom.table_y - 5.0;
    paint_rect(pix, w, ph, bx, by, 15.0, 5.0, (75, 38, 62), 0.78);
    paint_rect(pix, w, ph, bx + 1.0, by + 1.0, 6.0, 3.0, (210, 185, 130), 0.46);
    paint_rect(pix, w, ph, bx + 8.0, by + 1.0, 6.0, 3.0, (210, 185, 130), 0.46);
}

fn paint_phase_runes(pix: &mut Pix, w: usize, ph: usize, phase: LabPhase, phase_time: f64, mode: ColorMode) {
    let count = match phase {
        LabPhase::Idle => 1,
        LabPhase::Dropping => 2,
        LabPhase::Brewing => 4,
        LabPhase::Result => 6,
        LabPhase::Cooldown => 2,
    };

    let col = match phase {
        LabPhase::Idle => ambient_magic_color(mode),
        LabPhase::Dropping => spark_color(mode),
        LabPhase::Brewing => potion_color(mode),
        LabPhase::Result => (255, 245, 180),
        LabPhase::Cooldown => smoke_color(mode),
    };

    for i in 0..count {
        let pulse = ((phase_time * 3.0 + i as f64).sin() + 1.0) * 0.5;
        let x = 3.0 + i as f64 * 3.0;
        let y = 3.0;
        paint_soft_circle(pix, w, ph, x, y, 1.0, col, 0.45 + pulse * 0.30);
    }
}

fn paint_vignette(pix: &mut Pix, w: usize, ph: usize) {
    for y in 0..ph {
        for x in 0..w {
            let nx = (x as f64 / w.max(1) as f64 - 0.5).abs() * 2.0;
            let ny = (y as f64 / ph.max(1) as f64 - 0.5).abs() * 2.0;
            let d = ((nx * nx + ny * ny) * 0.5).clamp(0.0, 1.0);
            pix[y][x] = darken(pix[y][x], d.powf(2.0) * 0.30);
        }
    }
}

// ── Colors ───────────────────────────────────────────────────────────────────

fn potion_palette(mode: ColorMode) -> &'static [Rgb] {
    match mode {
        ColorMode::Ocean => &[(60, 190, 240), (80, 220, 210), (130, 180, 255), (160, 230, 245)],
        ColorMode::Sunset => &[(255, 130, 60), (255, 190, 75), (235, 70, 85), (255, 100, 140)],
        ColorMode::Matrix => &[(60, 255, 105), (30, 210, 70), (115, 255, 135), (180, 255, 90)],
        ColorMode::Rainbow => &[(190, 90, 255), (255, 90, 180), (90, 220, 255), (255, 210, 85)],
    }
}

fn potion_color(mode: ColorMode) -> Rgb {
    match mode {
        ColorMode::Ocean => (80, 205, 245),
        ColorMode::Sunset => (255, 125, 60),
        ColorMode::Matrix => (60, 255, 105),
        ColorMode::Rainbow => (190, 90, 255),
    }
}

fn spark_color(mode: ColorMode) -> Rgb {
    match mode {
        ColorMode::Ocean => (165, 235, 255),
        ColorMode::Sunset => (255, 210, 90),
        ColorMode::Matrix => (160, 255, 105),
        ColorMode::Rainbow => (255, 170, 245),
    }
}

fn smoke_color(mode: ColorMode) -> Rgb {
    match mode {
        ColorMode::Ocean => (110, 150, 170),
        ColorMode::Sunset => (130, 92, 76),
        ColorMode::Matrix => (55, 110, 70),
        ColorMode::Rainbow => (120, 95, 145),
    }
}

fn ambient_magic_color(mode: ColorMode) -> Rgb {
    match mode {
        ColorMode::Ocean => (100, 210, 255),
        ColorMode::Sunset => (255, 160, 80),
        ColorMode::Matrix => (80, 245, 120),
        ColorMode::Rainbow => (220, 120, 255),
    }
}

fn ingredient_color(kind: IngredientKind, mode: ColorMode) -> Rgb {
    let base = match kind {
        IngredientKind::Crystal => (100, 210, 255),
        IngredientKind::Leaf => (85, 210, 90),
        IngredientKind::Mushroom => (225, 80, 95),
        IngredientKind::StarDust => (255, 230, 110),
        IngredientKind::Eyeball => (160, 90, 220),
        IngredientKind::Feather => (210, 210, 235),
        IngredientKind::Flame => (255, 120, 50),
        IngredientKind::MoonDrop => (170, 205, 255),
    };

    match mode {
        ColorMode::Ocean => blend(base, (90, 210, 255), 0.12),
        ColorMode::Sunset => blend(base, (255, 120, 60), 0.12),
        ColorMode::Matrix => blend(base, (60, 255, 100), 0.16),
        ColorMode::Rainbow => base,
    }
}

fn lab_theme_tint(color_provider: &ColorProvider, base: Rgb, t: f64, x: i32, y: i32) -> Rgb {
    // Soft scene-grade, not a full rainbow wash.
    match color_provider.mode {
        ColorMode::Rainbow => {
            let pulse = ((t * 0.55 + (x + y) as f64 * 0.018).sin() + 1.0) * 0.5;
            blend(base, (205, 85, 255), 0.030 + pulse * 0.032)
        }
        ColorMode::Ocean => blend(base, (55, 145, 190), 0.11),
        ColorMode::Sunset => blend(base, (255, 125, 65), 0.11),
        ColorMode::Matrix => blend(base, (35, 220, 85), 0.15),
    }
}

// ── Primitive drawing ─────────────────────────────────────────────────────────

fn in_bounds(w: usize, ph: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < ph as i32
}

fn set_blend(pix: &mut Pix, w: usize, ph: usize, x: i32, y: i32, col: Rgb, alpha: f64) {
    if in_bounds(w, ph, x, y) {
        pix[y as usize][x as usize] = blend(pix[y as usize][x as usize], col, alpha);
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
                let lit = blend(col, (255, 255, 225), shade * 0.22);
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], lit, power);
            }
        }
    }
}

fn paint_soft_ellipse(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, rx: f64, ry: f64, col: Rgb, power: f64) {
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
            let d = (dx * dx + dy * dy).sqrt();

            if d <= 1.0 {
                let a = (1.0 - d).powf(1.5) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn paint_soft_circle(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, power: f64) {
    paint_soft_ellipse(pix, w, ph, cx, cy, r, r, col, power);
}

fn paint_ring(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, r: f64, col: Rgb, power: f64) {
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
            let edge = (d - r).abs();

            if edge <= 0.75 {
                let a = (1.0 - edge / 0.75).clamp(0.0, 1.0) * power;
                pix[py as usize][px as usize] =
                    blend(pix[py as usize][px as usize], col, a);
            }
        }
    }
}

fn paint_ring_ellipse(pix: &mut Pix, w: usize, ph: usize, cx: f64, cy: f64, rx: f64, ry: f64, col: Rgb, power: f64) {
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
            let d = ((dx * dx + dy * dy).sqrt() - 1.0).abs();

            if d < 0.11 {
                let a = (1.0 - d / 0.11).clamp(0.0, 1.0) * power;
                pix[py as usize][px as usize] = blend(pix[py as usize][px as usize], col, a);
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

            let fg = lab_theme_tint(color_provider, base_fg, t_abs, x as i32, upper as i32);
            let bg = lab_theme_tint(color_provider, base_bg, t_abs, x as i32, lower as i32);

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
