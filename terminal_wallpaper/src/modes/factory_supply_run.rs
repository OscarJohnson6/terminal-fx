// ===== src/modes/factory_supply_run.rs =====
//
// FactorySupplyRunMode
//
// Autonomous Rube-Goldberg factory wallpaper.
// Tiny courier bots carry parts across a moving factory floor, dodge presses,
// steam jets, magnets, saw gates, and conveyor belts, then deliver parts to
// an assembly station. Successful deliveries upgrade the factory. Losses matter
// because destroyed bots reduce the active workforce until the base rebuilds.
//
// Why this mode exists:
//   - Not another city/sky/space/water scene.
//   - Similar "stakes" to Ant Colony: leave safety -> risk hazards -> return reward.
//   - Adds a Rube-Goldberg / factory-machine visual language.
//
// Suggested registry:
//   use crate::modes::factory_supply_run::FactorySupplyRunMode;
//   mode_builder!(build_factory_supply_run, FactorySupplyRunMode);
//
//   impl ModeDescriptor for FactorySupplyRunMode {
//       const ID:   &'static str = "factory_run";
//       const NAME: &'static str = "Factory Run";
//       const DESC: &'static str = "Courier bots in a Rube-Goldberg factory";
//       const FPS:  u32          = 50;
//   }
//
// In register_all!:
//   factory_supply_run::FactorySupplyRunMode,
//
// In src/modes/mod.rs:
//   pub mod factory_supply_run;

use crate::ansi::{bg_rgb, rgb, RESET};
use crate::color::{ColorMode, ColorProvider};
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::TAU;

type Rgb = (u8, u8, u8);
type Pix = Vec<Vec<Rgb>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BotState {
    SpawnWait,
    ToParts,
    GrabPart,
    ToAssembler,
    Deliver,
    Returning,
    Stunned,
}

#[derive(Clone, Copy)]
struct Bot {
    x: f64,
    y: f64,
    vx: f64,
    state: BotState,
    carrying: bool,
    wait: f64,
    courage: f64,
    speed: f64,
    color: Rgb,
    phase: f64,
}

#[derive(Clone, Copy)]
enum HazardKind {
    Press,
    SteamJet,
    Magnet,
    SawGate,
}

#[derive(Clone, Copy)]
struct Hazard {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    phase: f64,
    period: f64,
    kind: HazardKind,
}

#[derive(Clone, Copy)]
enum ParticleKind {
    Spark,
    Steam,
    Bolt,
    Glow,
    Smoke,
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

pub struct FactorySupplyRunMode {
    speed: f64,
    color_provider: ColorProvider,

    time: f64,
    bots: Vec<Bot>,
    hazards: Vec<Hazard>,
    particles: Vec<Particle>,

    delivered: u32,
    lost: u32,
    parts_waiting: u32,
    products_built: u32,
    factory_level: u32,

    spawn_timer: f64,
    part_timer: f64,
    siren_timer: f64,

    last_dims: Option<(u16, u16)>,
}

impl FactorySupplyRunMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            bots: Vec::new(),
            hazards: Vec::new(),
            particles: Vec::new(),
            delivered: 0,
            lost: 0,
            parts_waiting: 5,
            products_built: 0,
            factory_level: 1,
            spawn_timer: 0.0,
            part_timer: 2.0,
            siren_timer: 0.0,
            last_dims: None,
        }
    }

    fn reset_scene(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        self.time = 0.0;
        self.bots.clear();
        self.hazards.clear();
        self.particles.clear();

        self.delivered = 0;
        self.lost = 0;
        self.parts_waiting = 6;
        self.products_built = 0;
        self.factory_level = 1;
        self.spawn_timer = 0.0;
        self.part_timer = 1.5;
        self.siren_timer = 0.0;

        let belt_y = belt_y(ph);
        let hazard_y = belt_y - 9.0;

        let slots = [
            (w * 0.25, HazardKind::Press),
            (w * 0.41, HazardKind::SteamJet),
            (w * 0.58, HazardKind::SawGate),
            (w * 0.73, HazardKind::Magnet),
        ];

        for (i, (x, kind)) in slots.iter().enumerate() {
            self.hazards.push(Hazard {
                x: *x,
                y: hazard_y + rng.random_range(-2.0..2.0),
                w: match kind {
                    HazardKind::Press => 11.0,
                    HazardKind::SteamJet => 9.0,
                    HazardKind::Magnet => 13.0,
                    HazardKind::SawGate => 10.0,
                },
                h: match kind {
                    HazardKind::Press => 14.0,
                    HazardKind::SteamJet => 18.0,
                    HazardKind::Magnet => 9.0,
                    HazardKind::SawGate => 12.0,
                },
                phase: rng.random_range(0.0..TAU) + i as f64,
                period: rng.random_range(2.2..4.0),
                kind: *kind,
            });
        }

        for i in 0..8 {
            self.spawn_bot(width, height, i as f64 * 0.25);
        }

        self.last_dims = Some((width, height));
    }

    fn spawn_bot(&mut self, width: u16, height: u16, wait: f64) {
        if self.bots.len() >= 22 {
            return;
        }

        let mut rng = rand::rng();
        let ph = height.max(1) as f64 * 2.0;
        let base = base_pos(width, height);

        let bot_color = match self.color_provider.mode {
            ColorMode::Matrix => (80, 255, 120),
            ColorMode::Ocean => (105, 210, 245),
            ColorMode::Sunset => (255, 170, 85),
            ColorMode::Rainbow => match rng.random_range(0..5) {
                0 => (255, 95, 125),
                1 => (95, 210, 255),
                2 => (255, 215, 95),
                3 => (150, 255, 130),
                _ => (220, 130, 255),
            },
        };

        self.bots.push(Bot {
            x: base.0 + rng.random_range(-2.0..2.0),
            y: belt_y(ph) - rng.random_range(2.0..5.0),
            vx: 0.0,
            state: BotState::SpawnWait,
            carrying: false,
            wait,
            courage: rng.random_range(0.55..1.25),
            speed: rng.random_range(15.0..28.0),
            color: bot_color,
            phase: rng.random_range(0.0..TAU),
        });
    }

    fn hazard_active(&self, h: Hazard) -> f64 {
        let t = ((self.time + h.phase) / h.period) % 1.0;

        match h.kind {
            HazardKind::Press => {
                if t < 0.18 {
                    1.0
                } else if t < 0.34 {
                    1.0 - (t - 0.18) / 0.16
                } else {
                    0.0
                }
            }
            HazardKind::SteamJet => {
                if (0.42..0.66).contains(&t) {
                    1.0
                } else {
                    0.0
                }
            }
            HazardKind::Magnet => {
                if (0.16..0.54).contains(&t) {
                    ((t - 0.16) / 0.38 * std::f64::consts::PI).sin().max(0.0)
                } else {
                    0.0
                }
            }
            HazardKind::SawGate => {
                if t < 0.48 {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }

    fn danger_near(&self, x: f64) -> f64 {
        let mut danger: f64 = 0.0;

        for h in &self.hazards {
            let active = self.hazard_active(*h);
            let d = ((x - h.x).abs() / (h.w + 7.0)).clamp(0.0, 1.0);
            danger = danger.max(active * (1.0 - d));
        }

        danger
    }

    fn spawn_particle(&mut self, kind: ParticleKind, x: f64, y: f64) {
        let mut rng = rand::rng();

        let (vx, vy, life, color) = match kind {
            ParticleKind::Spark => (
                rng.random_range(-8.0..8.0),
                rng.random_range(-12.0..-2.0),
                rng.random_range(0.35..0.95),
                (255, rng.random_range(145u8..230u8), 65),
            ),
            ParticleKind::Steam => (
                rng.random_range(-2.0..2.0),
                rng.random_range(-13.0..-4.0),
                rng.random_range(0.8..2.0),
                (145, 150, 150),
            ),
            ParticleKind::Bolt => (
                rng.random_range(-10.0..10.0),
                rng.random_range(-7.0..5.0),
                rng.random_range(0.25..0.65),
                (120, 210, 255),
            ),
            ParticleKind::Glow => (
                rng.random_range(-1.0..1.0),
                rng.random_range(-3.0..1.0),
                rng.random_range(0.6..1.4),
                (255, 235, 120),
            ),
            ParticleKind::Smoke => (
                rng.random_range(-2.0..2.0),
                rng.random_range(-7.0..-1.5),
                rng.random_range(1.0..2.4),
                (82, 78, 75),
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
        });
    }

    fn destroy_bot(&mut self, idx: usize, x: f64, y: f64) {
        self.lost += 1;
        self.siren_timer = 1.4;

        for _ in 0..18 {
            self.spawn_particle(ParticleKind::Spark, x, y);
        }

        for _ in 0..8 {
            self.spawn_particle(ParticleKind::Smoke, x, y);
        }

        self.bots.remove(idx);
    }

    fn deliver_part(&mut self, x: f64, y: f64) {
        self.delivered += 1;
        self.products_built += 1;

        if self.delivered % 7 == 0 {
            self.factory_level = (self.factory_level + 1).min(6);
            self.parts_waiting += 3;
            self.siren_timer = 0.8;

            for _ in 0..26 {
                self.spawn_particle(ParticleKind::Glow, x, y);
            }
        } else {
            for _ in 0..10 {
                self.spawn_particle(ParticleKind::Bolt, x, y);
            }
        }
    }
}


fn hazard_active_at(time: f64, h: Hazard) -> f64 {
    let t = ((time + h.phase) / h.period) % 1.0;

    match h.kind {
        HazardKind::Press => {
            if t < 0.18 {
                1.0
            } else if t < 0.34 {
                1.0 - (t - 0.18) / 0.16
            } else {
                0.0
            }
        }
        HazardKind::SteamJet => {
            if (0.42..0.66).contains(&t) {
                1.0
            } else {
                0.0
            }
        }
        HazardKind::Magnet => {
            if (0.16..0.54).contains(&t) {
                ((t - 0.16) / 0.38 * std::f64::consts::PI).sin().max(0.0)
            } else {
                0.0
            }
        }
        HazardKind::SawGate => {
            if t < 0.48 {
                1.0
            } else {
                0.0
            }
        }
    }
}

fn danger_near_from_hazards(active_hazards: &[(Hazard, f64)], x: f64) -> f64 {
    let mut danger: f64 = 0.0;

    for (h, active) in active_hazards {
        let d = ((x - h.x).abs() / (h.w + 7.0)).clamp(0.0, 1.0);
        danger = danger.max(*active * (1.0 - d));
    }

    danger
}


impl Mode for FactorySupplyRunMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.hazards.is_empty() {
            self.reset_scene(width, height);
        }

        let dt = (dt * self.speed).min(0.05);
        self.time += dt;
        self.siren_timer = (self.siren_timer - dt).max(0.0);

        let mut rng = rand::rng();
        let w = width.max(1) as f64;
        let ph = height.max(1) as f64 * 2.0;

        let base = base_pos(width, height);
        let parts = parts_pos(width, height);
        let assembler = assembler_pos(width, height);
        let belt = belt_y(ph);

        self.part_timer -= dt;
        if self.part_timer <= 0.0 {
            self.part_timer = rng.random_range(2.0..5.5);
            self.parts_waiting = (self.parts_waiting + rng.random_range(1..=3)).min(18);
            self.spawn_particle(ParticleKind::Glow, parts.0, parts.1);
        }

        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 && self.bots.len() < (8 + self.factory_level as usize * 2).min(20) {
            self.spawn_timer = rng.random_range(1.2..3.2);
            self.spawn_bot(width, height, 0.0);
        }

        // Ambient machine particles.
        for h in self.hazards.clone() {
            let active = self.hazard_active(h);
            if active > 0.6 {
                match h.kind {
                    HazardKind::SteamJet => {
                        for _ in 0..3 {
                            if rng.random_range(0.0..1.0) < 0.55 {
                                self.spawn_particle(ParticleKind::Steam, h.x, h.y + h.h * 0.35);
                            }
                        }
                    }
                    HazardKind::Magnet => {
                        if rng.random_range(0.0..1.0) < 0.35 {
                            self.spawn_particle(ParticleKind::Bolt, h.x, h.y);
                        }
                    }
                    HazardKind::Press | HazardKind::SawGate => {
                        if rng.random_range(0.0..1.0) < 0.22 {
                            self.spawn_particle(ParticleKind::Spark, h.x, h.y + h.h * 0.5);
                        }
                    }
                }
            }
        }

        let active_hazards: Vec<(Hazard, f64)> = self
            .hazards
            .iter()
            .copied()
            .map(|h| (h, hazard_active_at(self.time, h)))
            .collect();

        let mut i = 0usize;
        while i < self.bots.len() {
            let mut destroyed = false;
            let mut delivered = false;
            let mut delivered_x = 0.0;
            let mut delivered_y = 0.0;

            {
                let bot = &mut self.bots[i];

                bot.wait -= dt;
                bot.phase += dt * 8.0;

                if bot.wait > 0.0 {
                    i += 1;
                    continue;
                }

                let target_x = match bot.state {
                    BotState::SpawnWait => base.0,
                    BotState::ToParts => parts.0,
                    BotState::GrabPart => parts.0,
                    BotState::ToAssembler => assembler.0,
                    BotState::Deliver => assembler.0,
                    BotState::Returning => base.0,
                    BotState::Stunned => bot.x,
                };

                match bot.state {
                    BotState::SpawnWait => {
                        bot.state = if self.parts_waiting > 0 {
                            BotState::ToParts
                        } else {
                            BotState::Returning
                        };
                    }
                    BotState::ToParts => {
                        let danger = danger_near_from_hazards(&active_hazards, bot.x);
                        if danger > 0.55 && bot.courage < danger + 0.12 {
                            bot.vx *= 0.25;
                        } else {
                            bot.vx = (target_x - bot.x).signum() * bot.speed;
                        }

                        bot.x += bot.vx * dt;
                        bot.y = belt - 3.0 + (bot.phase).sin() * 0.6;

                        if (bot.x - parts.0).abs() < 2.2 {
                            bot.state = BotState::GrabPart;
                            bot.wait = rng.random_range(0.35..0.90);
                        }
                    }
                    BotState::GrabPart => {
                        if self.parts_waiting > 0 {
                            bot.carrying = true;
                            bot.state = BotState::ToAssembler;
                        } else {
                            bot.state = BotState::Returning;
                        }
                    }
                    BotState::ToAssembler => {
                        let danger = danger_near_from_hazards(&active_hazards, bot.x);
                        if danger > 0.45 && bot.courage < danger {
                            bot.vx *= 0.22;
                        } else {
                            bot.vx = (target_x - bot.x).signum() * bot.speed * if bot.carrying { 0.88 } else { 1.0 };
                        }

                        bot.x += bot.vx * dt;
                        bot.y = belt - 3.0 + (bot.phase).sin() * 0.6;

                        if (bot.x - assembler.0).abs() < 2.5 {
                            bot.state = BotState::Deliver;
                            bot.wait = rng.random_range(0.20..0.55);
                        }
                    }
                    BotState::Deliver => {
                        if bot.carrying {
                            bot.carrying = false;
                            delivered = true;
                            delivered_x = bot.x;
                            delivered_y = bot.y;
                        }

                        bot.state = BotState::Returning;
                    }
                    BotState::Returning => {
                        let danger = danger_near_from_hazards(&active_hazards, bot.x);
                        if danger > 0.65 && bot.courage < danger {
                            bot.vx *= 0.35;
                        } else {
                            bot.vx = (base.0 - bot.x).signum() * bot.speed * 1.05;
                        }

                        bot.x += bot.vx * dt;
                        bot.y = belt - 3.0 + (bot.phase).sin() * 0.6;

                        if (bot.x - base.0).abs() < 2.0 {
                            bot.state = if self.parts_waiting > 0 {
                                BotState::ToParts
                            } else {
                                BotState::SpawnWait
                            };
                            bot.wait = rng.random_range(0.35..1.1);
                        }
                    }
                    BotState::Stunned => {
                        bot.wait -= dt;
                        if bot.wait <= 0.0 {
                            bot.state = BotState::Returning;
                        }
                    }
                }

                // Conveyor drift.
                bot.x += (self.time * 1.7 + bot.phase).sin() * dt * 0.7;

                // Active hazard collision.
                for (h, active) in &active_hazards {
                    if *active < 0.45 {
                        continue;
                    }

                    let dx = (bot.x - h.x).abs();
                    let dy = (bot.y - (h.y + h.h * 0.45)).abs();
                    let hit_box = match h.kind {
                        HazardKind::Press => dx < h.w * 0.45 && dy < h.h * 0.30,
                        HazardKind::SteamJet => dx < h.w * 0.35 && dy < h.h * 0.55,
                        HazardKind::Magnet => dx < h.w * 0.95 && dy < h.h * 0.85 && bot.carrying,
                        HazardKind::SawGate => dx < h.w * 0.35 && dy < h.h * 0.35,
                    };

                    if hit_box {
                        let lethal = match h.kind {
                            HazardKind::Press => 0.72,
                            HazardKind::SawGate => 0.66,
                            HazardKind::SteamJet => 0.25,
                            HazardKind::Magnet => 0.20,
                        };

                        if rng.random_range(0.0..1.0) < lethal {
                            destroyed = true;
                        } else {
                            bot.state = BotState::Stunned;
                            bot.wait = rng.random_range(0.55..1.4);
                            bot.carrying = false;
                        }

                        break;
                    }
                }

                if bot.x < -18.0 || bot.x > w + 18.0 {
                    bot.x = base.0;
                    bot.state = BotState::SpawnWait;
                    bot.wait = rng.random_range(0.5..1.4);
                    bot.carrying = false;
                }
            }

            if delivered {
                self.parts_waiting = self.parts_waiting.saturating_sub(1);
                self.deliver_part(delivered_x, delivered_y);
            }

            if destroyed {
                let x = self.bots[i].x;
                let y = self.bots[i].y;
                self.destroy_bot(i, x, y);
            } else {
                i += 1;
            }
        }

        for p in &mut self.particles {
            p.x += p.vx * dt;
            p.y += p.vy * dt;

            match p.kind {
                ParticleKind::Spark | ParticleKind::Bolt => {
                    p.vy += 12.0 * dt;
                    p.vx *= 0.96;
                }
                ParticleKind::Steam | ParticleKind::Smoke => {
                    p.vy -= 1.8 * dt;
                    p.vx += (self.time * 2.0 + p.y * 0.06).sin() * dt * 1.5;
                }
                ParticleKind::Glow => {
                    p.vy -= 0.5 * dt;
                    p.vx *= 0.98;
                }
            }

            p.life -= dt;
        }

        self.particles.retain(|p| p.life > 0.0 && p.x > -30.0 && p.x < w + 30.0 && p.y > -30.0 && p.y < ph + 30.0);

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

        paint_background(&mut pix, w, ph, self.color_provider.mode, self.time, self.siren_timer);
        paint_back_machinery(&mut pix, w, ph, self.time, self.factory_level, self.color_provider.mode);
        paint_conveyors(&mut pix, w, ph, self.time, self.color_provider.mode);

        let base = base_pos(width, height);
        let parts = parts_pos(width, height);
        let assembler = assembler_pos(width, height);

        paint_base_station(&mut pix, w, ph, base, self.time, self.bots.len() as u32);
        paint_parts_bin(&mut pix, w, ph, parts, self.time, self.parts_waiting);
        paint_assembler(&mut pix, w, ph, assembler, self.time, self.products_built, self.factory_level);

        for hzd in &self.hazards {
            paint_hazard(&mut pix, w, ph, *hzd, self.hazard_active(*hzd), self.time, self.color_provider.mode);
        }

        for bot in &self.bots {
            paint_bot(&mut pix, w, ph, *bot, self.time);
        }

        for p in &self.particles {
            paint_particle(&mut pix, w, ph, *p);
        }

        paint_product_stack(&mut pix, w, ph, self.products_built, self.factory_level, self.time);
        paint_hud_lights(&mut pix, w, ph, self.delivered, self.lost, self.factory_level, self.time);
        paint_vignette(&mut pix, w, ph);

        half_blocks(&pix, w, h, &self.color_provider, t_abs)
    }
}

// ── Layout ───────────────────────────────────────────────────────────────────

fn belt_y(ph: f64) -> f64 {
    ph * 0.70
}

fn base_pos(width: u16, height: u16) -> (f64, f64) {
    let ph = height.max(1) as f64 * 2.0;
    (width.max(1) as f64 * 0.09, belt_y(ph) - 5.0)
}

fn parts_pos(width: u16, height: u16) -> (f64, f64) {
    let ph = height.max(1) as f64 * 2.0;
    (width.max(1) as f64 * 0.18, belt_y(ph) - 5.0)
}

fn assembler_pos(width: u16, height: u16) -> (f64, f64) {
    let ph = height.max(1) as f64 * 2.0;
    (width.max(1) as f64 * 0.91, belt_y(ph) - 6.0)
}

// ── Paint scene ───────────────────────────────────────────────────────────────

fn paint_background(pix: &mut Pix, w: usize, ph: usize, mode: ColorMode, time: f64, siren: f64) {
    let (top, bottom) = match mode {
        ColorMode::Ocean => ((8, 18, 30), (22, 54, 70)),
        ColorMode::Sunset => ((40, 18, 22), (86, 48, 30)),
        ColorMode::Matrix => ((0, 12, 7), (8, 36, 18)),
        ColorMode::Rainbow => ((15, 14, 28), (45, 36, 60)),
    };

    for y in 0..ph {
        let yf = y as f64 / ph.max(1) as f64;
        let mut col = lerp(top, bottom, yf.powf(0.85));
        let hum = ((time * 0.8 + y as f64 * 0.08).sin() + 1.0) * 0.5;
        col = blend(col, (255, 145, 70), hum * 0.025);

        if siren > 0.0 {
            let pulse = ((time * 16.0).sin() + 1.0) * 0.5;
            col = blend(col, (255, 30, 30), siren.min(1.0) * pulse * 0.16);
        }

        for x in 0..w {
            pix[y][x] = col;
        }
    }

    // wall grid panels
    for y in (ph / 7..ph * 3 / 5).step_by(8) {
        paint_rect(pix, w, ph, 0.0, y as f64, w as f64, 1.0, (50, 50, 58), 0.16);
    }

    for x in (0..w).step_by(18) {
        paint_rect(pix, w, ph, x as f64, ph as f64 * 0.12, 1.0, ph as f64 * 0.48, (50, 50, 58), 0.12);
    }
}

fn paint_back_machinery(pix: &mut Pix, w: usize, ph: usize, time: f64, level: u32, mode: ColorMode) {
    let pipe_col = match mode {
        ColorMode::Matrix => (22, 88, 42),
        _ => (62, 62, 68),
    };

    for i in 0..6 {
        let y = ph as f64 * (0.16 + i as f64 * 0.07);
        let offset = (time * (3.0 + i as f64) % 24.0) - 12.0;
        paint_rect(pix, w, ph, offset, y, w as f64 + 24.0, 2.0, pipe_col, 0.50);
        for x in (0..w).step_by(26) {
            paint_soft_circle(pix, w, ph, x as f64 + offset, y + 1.0, 2.0, darken(pipe_col, 0.25), 0.42);
        }
    }

    // Rotating gears.
    let gear_count = (3 + level.min(4)) as usize;
    for i in 0..gear_count {
        let x = w as f64 * (0.22 + i as f64 * 0.14);
        let y = ph as f64 * 0.34 + (i % 2) as f64 * 8.0;
        let r = 5.0 + (i % 3) as f64;
        paint_gear(pix, w, ph, x, y, r, time * (1.0 + i as f64 * 0.25), (78, 76, 78));
    }
}

fn paint_conveyors(pix: &mut Pix, w: usize, ph: usize, time: f64, mode: ColorMode) {
    let y = belt_y(ph as f64);
    let belt_col = match mode {
        ColorMode::Matrix => (12, 48, 28),
        ColorMode::Ocean => (34, 50, 62),
        ColorMode::Sunset => (58, 42, 36),
        ColorMode::Rainbow => (42, 38, 52),
    };

    paint_rect(pix, w, ph, 0.0, y, w as f64, 9.0, belt_col, 0.90);
    paint_rect(pix, w, ph, 0.0, y, w as f64, 2.0, (115, 112, 105), 0.60);
    paint_rect(pix, w, ph, 0.0, y + 8.0, w as f64, 2.0, (25, 24, 25), 0.60);

    for x in (0..w as i32).step_by(8) {
        let px = ((x as f64 + time * 28.0) % (w as f64 + 8.0)) - 8.0;
        draw_soft_line(pix, w, ph, px, y + 1.0, px + 5.0, y + 8.0, 0.75, (90, 88, 84), 0.55);
    }

    // lower return belt
    let y2 = ph as f64 * 0.87;
    paint_rect(pix, w, ph, 0.0, y2, w as f64, 5.0, darken(belt_col, 0.22), 0.62);
    for x in (0..w as i32).step_by(12) {
        let px = ((x as f64 - time * 18.0) % (w as f64 + 12.0)) - 12.0;
        paint_rect(pix, w, ph, px, y2 + 1.0, 6.0, 1.0, (95, 90, 84), 0.45);
    }
}

fn paint_base_station(pix: &mut Pix, w: usize, ph: usize, pos: (f64, f64), time: f64, bot_count: u32) {
    let (x, y) = pos;
    paint_rect(pix, w, ph, x - 10.0, y - 13.0, 20.0, 16.0, (36, 45, 50), 0.88);
    paint_rect(pix, w, ph, x - 8.0, y - 10.0, 16.0, 4.0, (55, 80, 92), 0.82);
    paint_rect(pix, w, ph, x - 5.0, y - 3.0, 10.0, 6.0, (18, 20, 24), 0.80);

    let pulse = ((time * 4.0).sin() + 1.0) * 0.5;
    let lights = bot_count.min(12);
    for i in 0..lights {
        let lx = x - 8.0 + i as f64 * 1.4;
        paint_soft_circle(pix, w, ph, lx, y - 8.0, 0.8, (95, 220, 255), 0.35 + pulse * 0.20);
    }
}

fn paint_parts_bin(pix: &mut Pix, w: usize, ph: usize, pos: (f64, f64), time: f64, parts: u32) {
    let (x, y) = pos;

    paint_rect(pix, w, ph, x - 8.0, y - 8.0, 16.0, 10.0, (68, 54, 42), 0.86);
    paint_rect(pix, w, ph, x - 7.0, y - 7.0, 14.0, 2.0, (130, 95, 58), 0.72);

    let visible = parts.min(14);
    for i in 0..visible {
        let px = x - 6.0 + (i % 7) as f64 * 2.0;
        let py = y - 5.0 + (i / 7) as f64 * 2.2;
        let pulse = ((time * 2.0 + i as f64).sin() + 1.0) * 0.5;
        paint_soft_circle(pix, w, ph, px, py, 1.1, (235, 175, 75), 0.55 + pulse * 0.12);
    }
}

fn paint_assembler(pix: &mut Pix, w: usize, ph: usize, pos: (f64, f64), time: f64, products: u32, level: u32) {
    let (x, y) = pos;
    let pulse = ((time * 5.0).sin() + 1.0) * 0.5;

    paint_rect(pix, w, ph, x - 12.0, y - 18.0, 24.0, 21.0, (44, 44, 50), 0.91);
    paint_rect(pix, w, ph, x - 9.0, y - 15.0, 18.0, 6.0, (70, 78, 85), 0.78);
    paint_rect(pix, w, ph, x - 6.0, y - 6.0, 12.0, 7.0, (14, 18, 20), 0.88);

    let glow = if products > 0 { 0.20 + pulse * 0.16 } else { 0.08 };
    paint_soft_circle(pix, w, ph, x, y - 8.0, 12.0 + level as f64, (120, 220, 255), glow * 0.32);

    for i in 0..level.min(6) {
        paint_soft_circle(pix, w, ph, x - 7.0 + i as f64 * 2.8, y - 12.0, 1.0, (255, 220, 95), 0.50 + pulse * 0.25);
    }
}

fn paint_hazard(pix: &mut Pix, w: usize, ph: usize, h: Hazard, active: f64, time: f64, mode: ColorMode) {
    let warning = match mode {
        ColorMode::Matrix => (80, 255, 120),
        ColorMode::Ocean => (90, 210, 255),
        ColorMode::Sunset => (255, 150, 60),
        ColorMode::Rainbow => (255, 95, 150),
    };

    match h.kind {
        HazardKind::Press => {
            let drop = active * 9.0;
            paint_rect(pix, w, ph, h.x - h.w * 0.5, h.y - 9.0, h.w, 6.0 + drop, (86, 80, 78), 0.88);
            paint_rect(pix, w, ph, h.x - h.w * 0.65, h.y - 3.0 + drop, h.w * 1.3, 4.0, (145, 135, 120), 0.85);
            if active > 0.3 {
                paint_soft_circle(pix, w, ph, h.x, h.y + drop, h.w, warning, active * 0.10);
            }
        }
        HazardKind::SteamJet => {
            paint_rect(pix, w, ph, h.x - 3.0, h.y + h.h * 0.5, 6.0, 8.0, (72, 70, 64), 0.80);
            if active > 0.2 {
                paint_soft_circle(pix, w, ph, h.x, h.y + h.h * 0.1, 7.0, (170, 175, 170), active * 0.22);
                paint_soft_circle(pix, w, ph, h.x, h.y - 3.0, 11.0, (170, 175, 170), active * 0.10);
            }
        }
        HazardKind::Magnet => {
            paint_rect(pix, w, ph, h.x - 6.0, h.y - 3.0, 4.0, 10.0, (155, 45, 50), 0.82);
            paint_rect(pix, w, ph, h.x + 2.0, h.y - 3.0, 4.0, 10.0, (55, 90, 180), 0.82);
            paint_rect(pix, w, ph, h.x - 6.0, h.y - 3.0, 12.0, 3.0, (92, 88, 95), 0.75);

            if active > 0.05 {
                for i in 0..5 {
                    let r = 5.0 + i as f64 * 3.0 + (time * 3.0).sin();
                    paint_soft_circle(pix, w, ph, h.x, h.y + 1.0, r, warning, active * 0.025);
                }
            }
        }
        HazardKind::SawGate => {
            let spin = time * 8.0 + h.phase;
            paint_soft_circle(pix, w, ph, h.x, h.y + 3.0, 5.5, (150, 150, 150), 0.82);
            for i in 0..8 {
                let a = spin + TAU * i as f64 / 8.0;
                draw_soft_line(
                    pix,
                    w,
                    ph,
                    h.x,
                    h.y + 3.0,
                    h.x + a.cos() * 7.0,
                    h.y + 3.0 + a.sin() * 7.0,
                    0.7,
                    (205, 205, 195),
                    0.75,
                );
            }

            if active > 0.2 {
                paint_soft_circle(pix, w, ph, h.x, h.y + 3.0, 12.0, warning, active * 0.06);
            }
        }
    }
}

fn paint_bot(pix: &mut Pix, w: usize, ph: usize, bot: Bot, time: f64) {
    let bob = (time * 8.0 + bot.phase).sin() * 0.5;
    let x = bot.x;
    let y = bot.y + bob;

    let body = if matches!(bot.state, BotState::Stunned) {
        blend(bot.color, (255, 255, 255), 0.45)
    } else {
        bot.color
    };

    paint_soft_circle(pix, w, ph, x, y, 2.2, body, 0.88);
    paint_rect(pix, w, ph, x - 1.5, y - 1.0, 3.0, 2.0, body, 0.82);

    // eye
    paint_soft_circle(pix, w, ph, x + 0.9, y - 0.6, 0.6, (245, 250, 255), 0.82);

    // little legs
    let leg = (time * 12.0 + bot.phase).sin();
    paint_soft_circle(pix, w, ph, x - 1.1, y + 2.0 + leg * 0.6, 0.5, (20, 20, 22), 0.82);
    paint_soft_circle(pix, w, ph, x + 1.1, y + 2.0 - leg * 0.6, 0.5, (20, 20, 22), 0.82);

    if bot.carrying {
        paint_soft_circle(pix, w, ph, x, y - 3.4, 1.3, (255, 190, 80), 0.86);
        paint_soft_circle(pix, w, ph, x, y - 3.4, 3.8, (255, 190, 80), 0.06);
    }
}

fn paint_particle(pix: &mut Pix, w: usize, ph: usize, p: Particle) {
    let fade = (p.life / p.max_life).clamp(0.0, 1.0);

    match p.kind {
        ParticleKind::Spark | ParticleKind::Bolt => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.2, p.color, fade * 0.68);
            paint_soft_circle(pix, w, ph, p.x, p.y, 3.6, p.color, fade * 0.07);
        }
        ParticleKind::Steam | ParticleKind::Smoke => {
            let r = 2.0 + (1.0 - fade) * 5.0;
            paint_soft_circle(pix, w, ph, p.x, p.y, r, p.color, fade * 0.18);
        }
        ParticleKind::Glow => {
            paint_soft_circle(pix, w, ph, p.x, p.y, 1.4, p.color, fade * 0.44);
            paint_soft_circle(pix, w, ph, p.x, p.y, 7.0, p.color, fade * 0.08);
        }
    }
}

fn paint_product_stack(pix: &mut Pix, w: usize, ph: usize, products: u32, level: u32, time: f64) {
    let count = products.min(30);
    let base_x = w as f64 * 0.92;
    let base_y = ph as f64 * 0.89;

    for i in 0..count {
        let row = i / 6;
        let col = i % 6;
        let x = base_x - col as f64 * 3.5;
        let y = base_y - row as f64 * 3.0;
        let pulse = ((time * 1.7 + i as f64).sin() + 1.0) * 0.5;
        let color = match level % 4 {
            0 => (120, 220, 255),
            1 => (255, 195, 80),
            2 => (130, 255, 150),
            _ => (255, 120, 180),
        };
        paint_rect(pix, w, ph, x, y, 2.6, 2.0, color, 0.55 + pulse * 0.10);
    }
}

fn paint_hud_lights(pix: &mut Pix, w: usize, ph: usize, delivered: u32, lost: u32, level: u32, time: f64) {
    let y = 3.0;
    let mut x = 4.0;

    for i in 0..level.min(6) {
        let pulse = ((time * 3.0 + i as f64).sin() + 1.0) * 0.5;
        paint_soft_circle(pix, w, ph, x, y, 1.2, (100, 240, 120), 0.55 + pulse * 0.25);
        x += 3.0;
    }

    x += 4.0;
    for i in 0..delivered.min(18) {
        paint_soft_circle(pix, w, ph, x + i as f64 * 1.8, y, 0.7, (255, 210, 90), 0.55);
    }

    let lost_x = w as f64 - 4.0;
    for i in 0..lost.min(10) {
        paint_soft_circle(pix, w, ph, lost_x - i as f64 * 1.8, y, 0.8, (255, 60, 60), 0.55);
    }
}

fn paint_gear(pix: &mut Pix, w: usize, ph: usize, x: f64, y: f64, r: f64, angle: f64, color: Rgb) {
    paint_soft_circle(pix, w, ph, x, y, r, color, 0.36);
    paint_soft_circle(pix, w, ph, x, y, r * 0.35, (18, 18, 20), 0.75);

    for i in 0..10 {
        let a = angle + TAU * i as f64 / 10.0;
        let tx = x + a.cos() * r * 1.05;
        let ty = y + a.sin() * r * 1.05;
        paint_soft_circle(pix, w, ph, tx, ty, 1.2, color, 0.42);
    }
}

fn paint_vignette(pix: &mut Pix, w: usize, ph: usize) {
    for y in 0..ph {
        for x in 0..w {
            let nx = (x as f64 / w.max(1) as f64 - 0.5).abs() * 2.0;
            let ny = (y as f64 / ph.max(1) as f64 - 0.5).abs() * 2.0;
            let d = ((nx * nx + ny * ny) * 0.5).clamp(0.0, 1.0);
            pix[y][x] = darken(pix[y][x], d.powf(2.1) * 0.23);
        }
    }
}

// ── Primitive drawing ─────────────────────────────────────────────────────────

fn in_bounds(w: usize, ph: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < ph as i32
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

fn factory_theme_tint(color_provider: &ColorProvider, base: Rgb, t: f64, x: i32, y: i32) -> Rgb {
    match color_provider.mode {
        ColorMode::Rainbow => {
            let pulse = ((t * 0.8 + (x + y) as f64 * 0.018).sin() + 1.0) * 0.5;
            blend(base, (210, 100, 255), 0.04 + pulse * 0.035)
        }
        ColorMode::Ocean => blend(base, (45, 130, 170), 0.14),
        ColorMode::Sunset => blend(base, (255, 120, 55), 0.14),
        ColorMode::Matrix => blend(base, (35, 210, 85), 0.18),
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

            let fg = factory_theme_tint(color_provider, base_fg, t_abs, x as i32, upper as i32);
            let bg = factory_theme_tint(color_provider, base_bg, t_abs, x as i32, lower as i32);

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
