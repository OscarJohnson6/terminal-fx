// ===== src/modes/spirit_grove.rs =====
//
// SpiritGroveMode
//
// A tree-inspired mode, but not just "one tree again":
//   - Several trees grow into a small grove.
//   - The grove cycles through spring growth, mature firefly night,
//     autumn leaf fall, quiet winter, then reseeds.
//   - Tiny animals occasionally cross the ground.
//   - Fireflies, falling leaves, moon/sun glow, and wind sway give it life.
//
// Style:
//   - Mostly symbolic / ASCII like the original tree mode.
//   - Uses full-cell characters, not half-block pixels.
//   - Theme changes the grove mood, but does not rainbow-wash the whole scene.
//
// Suggested registry:
//
// In src/modes/mod.rs:
//   pub mod spirit_grove;
//
// In mode_registry.rs descriptor imports:
//   spirit_grove::SpiritGroveMode,
//
// Descriptor:
//   impl ModeDescriptor for SpiritGroveMode {
//       const ID:   &'static str = "spirit_grove";
//       const NAME: &'static str = "Spirit Grove";
//       const DESC: &'static str = "Growing grove with fireflies and seasons";
//       const FPS:  u32          = 50;
//   }
//
// In register_all!:
//   spirit_grove::SpiritGroveMode,

use crate::ansi::{rgb, RESET};
use crate::color::{ColorMode, ColorProvider};
use crate::mode_base::Mode;
use rand::RngExt;
use std::f64::consts::{FRAC_PI_2, TAU};

#[derive(Clone, Copy)]
struct Seg {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    depth: u32,
    start_t: f64,
    duration: f64,
}

#[derive(Clone, Copy)]
struct GroveTree {
    base_x: f64,
    base_y: f64,
    seed_t: f64,
    total_t: f64,
    height_scale: f64,
    lean: f64,
    hue_shift: f64,
}

#[derive(Clone, Copy)]
struct Firefly {
    x: f64,
    y: f64,
    vx: f64,
    phase: f64,
}

#[derive(Clone, Copy)]
struct Leaf {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    spin: f64,
    color: (u8, u8, u8),
}

#[derive(Clone, Copy)]
struct Animal {
    x: f64,
    y: f64,
    vx: f64,
    kind: AnimalKind,
    phase: f64,
}

#[derive(Clone, Copy)]
enum AnimalKind {
    Fox,
    Deer,
    Rabbit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Season {
    Spring,
    Summer,
    Autumn,
    Winter,
}

pub struct SpiritGroveMode {
    speed: f64,
    color_provider: ColorProvider,

    time: f64,
    season_time: f64,
    season: Season,

    trees: Vec<GroveTree>,
    segs: Vec<Vec<Seg>>,
    fireflies: Vec<Firefly>,
    leaves: Vec<Leaf>,
    animals: Vec<Animal>,

    ground_y: f64,
    last_dims: Option<(u16, u16)>,
}

impl SpiritGroveMode {
    pub fn new(speed: f64, color_provider: ColorProvider) -> Self {
        Self {
            speed,
            color_provider,
            time: 0.0,
            season_time: 0.0,
            season: Season::Spring,
            trees: Vec::new(),
            segs: Vec::new(),
            fireflies: Vec::new(),
            leaves: Vec::new(),
            animals: Vec::new(),
            ground_y: 20.0,
            last_dims: None,
        }
    }

    fn reset_grove(&mut self, width: u16, height: u16) {
        let mut rng = rand::rng();

        self.time = 0.0;
        self.season_time = 0.0;
        self.season = Season::Spring;
        self.ground_y = height.max(4) as f64 - 3.0;

        self.trees.clear();
        self.segs.clear();
        self.fireflies.clear();
        self.leaves.clear();
        self.animals.clear();

        let w = width.max(20) as f64;
        let h = height.max(10) as f64;
        let tree_count = if width < 70 { 4 } else { 6 };

        for i in 0..tree_count {
            let frac = (i as f64 + 0.55) / tree_count as f64;
            let base_x = frac * w + rng.random_range(-w * 0.045..w * 0.045);
            let scale = rng.random_range(0.72..1.08) * (h / 28.0).clamp(0.72, 1.35);
            let lean = rng.random_range(-0.17..0.17);

            let tree = GroveTree {
                base_x: base_x.clamp(4.0, w - 4.0),
                base_y: self.ground_y,
                seed_t: i as f64 * rng.random_range(0.55..1.05),
                total_t: 0.0,
                height_scale: scale,
                lean,
                hue_shift: rng.random_range(0.0..1.0),
            };

            let mut tree_segs = Vec::new();
            let trunk_len = h * rng.random_range(0.22..0.34) * scale;

            gen_tree(
                &mut tree_segs,
                tree.base_x,
                tree.base_y,
                -FRAC_PI_2 + lean,
                trunk_len,
                0,
                0.0,
                0.0,
                &mut rng,
            );

            let total_t = tree_segs
                .iter()
                .map(|s| s.start_t + s.duration)
                .fold(0.0_f64, f64::max);

            self.trees.push(GroveTree { total_t, ..tree });
            self.segs.push(tree_segs);
        }

        for _ in 0..32 {
            self.fireflies.push(Firefly {
                x: rng.random_range(0.0..w),
                y: rng.random_range(h * 0.22..self.ground_y - 2.0),
                vx: rng.random_range(-4.0..4.0),
                phase: rng.random_range(0.0..TAU),
            });
        }

        self.last_dims = Some((width, height));
    }

    fn next_season(&mut self) {
        self.season_time = 0.0;

        self.season = match self.season {
            Season::Spring => Season::Summer,
            Season::Summer => Season::Autumn,
            Season::Autumn => Season::Winter,
            Season::Winter => Season::Spring,
        };

        if self.season == Season::Spring {
            // Spring is a real reseed cycle. The trees are regenerated, but the
            // current terminal size is preserved by update() calling reset_grove.
            self.last_dims = None;
        }
    }

    fn spawn_leaf(&mut self, width: u16) {
        if self.leaves.len() > 150 || self.segs.is_empty() {
            return;
        }

        let mut rng = rand::rng();
        let tree_i = rng.random_range(0..self.segs.len());
        let tips: Vec<&Seg> = self.segs[tree_i].iter().filter(|s| s.depth >= 6).collect();

        if tips.is_empty() {
            return;
        }

        let tip = tips[rng.random_range(0..tips.len())];

        let color = match rng.random_range(0..5) {
            0 => (230, 150, 55),
            1 => (210, 95, 45),
            2 => (190, 130, 45),
            3 => (230, 185, 70),
            _ => (160, 85, 45),
        };

        self.leaves.push(Leaf {
            x: tip.x2.clamp(0.0, width.max(1) as f64 - 1.0),
            y: tip.y2,
            vx: rng.random_range(-4.0..4.0),
            vy: rng.random_range(2.5..7.0),
            spin: rng.random_range(0.0..TAU),
            color,
        });
    }

    fn maybe_spawn_animal(&mut self, width: u16) {
        if self.animals.len() > 2 {
            return;
        }

        let mut rng = rand::rng();
        if rng.random_range(0.0..1.0) > 0.012 {
            return;
        }

        let going_right = rng.random_bool(0.5);
        let kind = match rng.random_range(0..3) {
            0 => AnimalKind::Fox,
            1 => AnimalKind::Rabbit,
            _ => AnimalKind::Deer,
        };

        let speed = match kind {
            AnimalKind::Fox => rng.random_range(7.0..13.0),
            AnimalKind::Rabbit => rng.random_range(9.0..17.0),
            AnimalKind::Deer => rng.random_range(5.0..9.0),
        };

        self.animals.push(Animal {
            x: if going_right { -5.0 } else { width.max(1) as f64 + 5.0 },
            y: self.ground_y - 1.0,
            vx: if going_right { speed } else { -speed },
            kind,
            phase: rng.random_range(0.0..TAU),
        });
    }
}

impl Mode for SpiritGroveMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if self.last_dims != Some((width, height)) || self.trees.is_empty() {
            self.reset_grove(width, height);
        }

        let dt = (dt * self.speed).min(0.06);
        self.time += dt;
        self.season_time += dt;

        match self.season {
            Season::Spring if self.season_time > 15.0 => self.next_season(),
            Season::Summer if self.season_time > 16.0 => self.next_season(),
            Season::Autumn if self.season_time > 13.0 => self.next_season(),
            Season::Winter if self.season_time > 8.0 => self.next_season(),
            _ => {}
        }

        let w = width.max(1) as f64;

        if self.season == Season::Autumn {
            let mut rng = rand::rng();
            for _ in 0..4 {
                if rng.random_range(0.0..1.0) < 0.45 {
                    self.spawn_leaf(width);
                }
            }
        }

        for leaf in &mut self.leaves {
            leaf.spin += dt * 5.0;
            leaf.vx += (self.time * 1.6 + leaf.y * 0.07).sin() * dt * 4.2;
            leaf.x += leaf.vx * dt;
            leaf.y += leaf.vy * dt;
        }

        let ground = self.ground_y;
        self.leaves
            .retain(|leaf| leaf.y < ground + 0.5 && leaf.x > -3.0 && leaf.x < w + 3.0);

        for f in &mut self.fireflies {
            f.phase += dt * 2.0;
            f.x += (f.vx + (self.time * 1.3 + f.phase).sin() * 3.0) * dt;
            f.y += (self.time * 0.9 + f.phase).cos() * dt * 2.0;

            if f.x < -2.0 {
                f.x = w + 2.0;
            } else if f.x > w + 2.0 {
                f.x = -2.0;
            }

            let top = height.max(5) as f64 * 0.18;
            f.y = f.y.clamp(top, self.ground_y - 2.0);
        }

        self.maybe_spawn_animal(width);

        for a in &mut self.animals {
            a.phase += dt * 8.0;
            a.x += a.vx * dt;
        }

        self.animals
            .retain(|a| a.x > -10.0 && a.x < width.max(1) as f64 + 10.0);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width.max(1) as usize;
        let h = height.max(1) as usize;

        let mut buf = vec![vec![" ".to_string(); w]; h];

        self.paint_sky(&mut buf, w, h, t_abs);
        self.paint_ground(&mut buf, w, h, t_abs);
        self.paint_moon_or_sun(&mut buf, w, h, t_abs);

        let grove_anim_t = match self.season {
            Season::Spring => self.season_time,
            _ => 999.0,
        };

        for (i, tree) in self.trees.iter().enumerate() {
            let local_t = grove_anim_t - tree.seed_t;

            if local_t < 0.0 {
                self.paint_seed(&mut buf, w, h, tree.base_x, tree.base_y, i);
                continue;
            }

            self.paint_tree(&mut buf, w, h, i, *tree, local_t, t_abs);
        }

        for leaf in &self.leaves {
            let x = leaf.x.round() as i32;
            let y = leaf.y.round() as i32;
            if in_bounds(w, h, x, y) {
                let chars = ['🍂', '·', '*', '✦'];
                let ch = chars[((leaf.spin * 2.0) as usize) % chars.len()];
                buf[y as usize][x as usize] = format!(
                    "{}{}{}",
                    rgb(leaf.color.0, leaf.color.1, leaf.color.2),
                    ch,
                    RESET
                );
            }
        }

        if matches!(self.season, Season::Summer | Season::Autumn) {
            self.paint_fireflies(&mut buf, w, h, t_abs);
        }

        for animal in &self.animals {
            self.paint_animal(&mut buf, w, h, *animal);
        }

        self.paint_season_indicator(&mut buf, w, h, t_abs);

        buf.into_iter()
            .map(|r| r.join(""))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ── Tree generation ──────────────────────────────────────────────────────────

fn gen_tree(
    segs: &mut Vec<Seg>,
    x: f64,
    y: f64,
    angle: f64,
    len: f64,
    depth: u32,
    parent_start: f64,
    parent_dur: f64,
    rng: &mut impl RngExt,
) {
    if depth > 8 || len < 1.05 {
        return;
    }

    let start_t = if depth == 0 {
        0.0
    } else {
        parent_start + parent_dur * rng.random_range(0.50..0.78)
    };

    let duration = (len / (8.5 + depth as f64)).max(0.10);
    let ex = x + angle.cos() * len;
    let ey = y + angle.sin() * len;

    segs.push(Seg {
        x1: x,
        y1: y,
        x2: ex,
        y2: ey,
        depth,
        start_t,
        duration,
    });

    let child_count = match depth {
        0 => 2,
        1 | 2 => rng.random_range(2usize..=3),
        3 | 4 => rng.random_range(1usize..=3),
        _ => rng.random_range(1usize..=2),
    };

    let spread = rng.random_range(0.38..0.78);

    for i in 0..child_count {
        let frac = if child_count == 1 {
            0.5
        } else {
            i as f64 / (child_count - 1) as f64
        };

        let off = (frac - 0.5) * spread * 2.0 + rng.random_range(-0.11..0.11);
        let child_len = len * rng.random_range(0.58..0.78);

        gen_tree(
            segs,
            ex,
            ey,
            angle + off,
            child_len,
            depth + 1,
            start_t,
            duration,
            rng,
        );
    }
}

// ── Rendering helpers ────────────────────────────────────────────────────────

impl SpiritGroveMode {
    fn paint_sky(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, t_abs: f64) {
        let (top, bottom) = match (self.season, self.color_provider.mode) {
            (Season::Winter, _) => ((18, 24, 42), (62, 72, 88)),
            (_, ColorMode::Ocean) => ((4, 34, 60), (20, 82, 105)),
            (_, ColorMode::Sunset) => ((54, 22, 48), (150, 80, 58)),
            (_, ColorMode::Matrix) => ((0, 18, 10), (8, 58, 28)),
            _ => ((10, 24, 55), (60, 95, 115)),
        };

        for y in 0..h {
            let t = y as f64 / h.max(1) as f64;
            let mut col = lerp_rgb(top, bottom, t.powf(0.82));

            let breeze = ((t_abs * 0.25 + y as f64 * 0.15).sin() + 1.0) * 0.5;
            col = blend(col, (255, 160, 95), breeze * 0.025);

            for x in 0..w {
                let star = self.season == Season::Summer
                    && y < h / 3
                    && ((x * 37 + y * 17) % 151 == 0);

                if star {
                    buf[y][x] = format!("{}·{}", rgb(190, 210, 220), RESET);
                } else {
                    buf[y][x] = format!("{} {}", rgb(col.0, col.1, col.2), RESET);
                }
            }
        }
    }

    fn paint_ground(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, t_abs: f64) {
        let ground = self.ground_y.round().clamp(0.0, h.saturating_sub(1) as f64) as usize;

        for y in ground..h {
            for x in 0..w {
                let n = ((x as f64 * 0.31 + y as f64 * 0.17 + t_abs * 0.1).sin() + 1.0) * 0.5;

                let col = match self.season {
                    Season::Spring => blend((44, 95, 36), (90, 150, 55), n * 0.25),
                    Season::Summer => blend((38, 105, 35), (80, 135, 40), n * 0.25),
                    Season::Autumn => blend((82, 62, 34), (150, 92, 38), n * 0.25),
                    Season::Winter => blend((70, 82, 88), (170, 180, 185), n * 0.34),
                };

                let ch = if y == ground {
                    match self.season {
                        Season::Winter => '░',
                        Season::Autumn => if x % 5 == 0 { '·' } else { '▓' },
                        _ => if x % 7 == 0 { '\'' } else { '▓' },
                    }
                } else {
                    '▓'
                };

                buf[y][x] = format!("{}{}{}", rgb(col.0, col.1, col.2), ch, RESET);
            }
        }
    }

    fn paint_moon_or_sun(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, t_abs: f64) {
        if w < 12 || h < 8 {
            return;
        }

        let x = match self.season {
            Season::Spring | Season::Summer => w as i32 - 9,
            Season::Autumn | Season::Winter => 8,
        };
        let y = 3i32;

        let col = match self.season {
            Season::Spring => (255, 235, 145),
            Season::Summer => (255, 220, 115),
            Season::Autumn => (255, 160, 90),
            Season::Winter => (200, 220, 245),
        };

        let pulse = ((t_abs * 0.7).sin() + 1.0) * 0.5;
        for dy in -2..=2 {
            for dx in -4..=4 {
                let d = ((dx as f64 / 2.0).powi(2) + (dy as f64).powi(2)).sqrt();
                let px = x + dx;
                let py = y + dy;
                if in_bounds(w, h, px, py) && d < 2.2 {
                    let ch = if d < 1.15 { '●' } else { '·' };
                    let c = blend(col, (255, 255, 255), pulse * 0.12);
                    buf[py as usize][px as usize] = format!("{}{}{}", rgb(c.0, c.1, c.2), ch, RESET);
                }
            }
        }
    }

    fn paint_seed(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, x: f64, y: f64, idx: usize) {
        let sx = x.round() as i32;
        let sy = y.round() as i32;
        if in_bounds(w, h, sx, sy) {
            let pulse = ((self.time * 4.0 + idx as f64).sin() + 1.0) * 0.5;
            let col = blend((130, 90, 42), (220, 160, 70), pulse * 0.35);
            buf[sy as usize][sx as usize] = format!("{}◉{}", rgb(col.0, col.1, col.2), RESET);
        }
    }

    fn paint_tree(
        &self,
        buf: &mut Vec<Vec<String>>,
        w: usize,
        h: usize,
        tree_i: usize,
        tree: GroveTree,
        local_t: f64,
        t_abs: f64,
    ) {
        let Some(segs) = self.segs.get(tree_i) else {
            return;
        };

        for seg in segs {
            if local_t < seg.start_t {
                continue;
            }

            let progress = ((local_t - seg.start_t) / seg.duration).clamp(0.0, 1.0);
            if progress <= 0.01 {
                continue;
            }

            let mut x2 = seg.x1 + (seg.x2 - seg.x1) * progress;
            let y2 = seg.y1 + (seg.y2 - seg.y1) * progress;

            let sway_amount = match self.season {
                Season::Spring => 0.25,
                Season::Summer => 0.55,
                Season::Autumn => 0.80,
                Season::Winter => 0.18,
            };

            if seg.depth >= 3 {
                x2 += (t_abs * 1.6 + seg.y1 * 0.19 + tree.hue_shift * 5.0).sin()
                    * sway_amount
                    * (seg.depth as f64 - 2.0)
                    * 0.18;
            }

            let col = branch_color(seg.depth, self.season, self.color_provider.mode, tree.hue_shift);
            let ch = branch_char(seg.x2 - seg.x1, seg.y2 - seg.y1, seg.depth, self.season);

            draw_seg(buf, w, h, seg.x1, seg.y1, x2, y2, col, ch);
        }

        if local_t > tree.total_t * 0.72 {
            self.paint_canopy(buf, w, h, segs, local_t, t_abs, tree.hue_shift);
        }
    }

    fn paint_canopy(
        &self,
        buf: &mut Vec<Vec<String>>,
        w: usize,
        h: usize,
        segs: &[Seg],
        local_t: f64,
        t_abs: f64,
        hue_shift: f64,
    ) {
        let reveal = ((local_t - 4.0) / 5.0).clamp(0.0, 1.0);
        if reveal <= 0.0 {
            return;
        }

        let leaf_chars = match self.season {
            Season::Spring => ['✿', '·', '*', '✿'],
            Season::Summer => ['✦', '*', '·', '◦'],
            Season::Autumn => ['*', '·', '✦', '·'],
            Season::Winter => ['·', '·', '*', '·'],
        };

        for seg in segs.iter().filter(|s| s.depth >= 6) {
            let x = seg.x2.round() as i32;
            let y = seg.y2.round() as i32;

            if !in_bounds(w, h, x, y) {
                continue;
            }

            let gate = ((seg.x1 * 12.31 + seg.y1 * 7.77 + hue_shift * 99.0).sin() + 1.0) * 0.5;
            if gate > reveal {
                continue;
            }

            let idx = ((t_abs * 1.4 + seg.x1 * 0.31 + seg.y1 * 0.17) as usize) % leaf_chars.len();
            let col = leaf_color(self.season, self.color_provider.mode, hue_shift, gate);
            buf[y as usize][x as usize] = format!("{}{}{}", rgb(col.0, col.1, col.2), leaf_chars[idx], RESET);
        }
    }

    fn paint_fireflies(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, t_abs: f64) {
        for f in &self.fireflies {
            let x = f.x.round() as i32;
            let y = f.y.round() as i32;

            if !in_bounds(w, h, x, y) {
                continue;
            }

            let pulse = ((t_abs * 3.4 + f.phase).sin() + 1.0) * 0.5;
            if pulse < 0.32 {
                continue;
            }

            let col = match self.color_provider.mode {
                ColorMode::Matrix => (80, 255, 120),
                ColorMode::Ocean => (120, 225, 255),
                ColorMode::Sunset => (255, 210, 110),
                ColorMode::Rainbow => (230, 255, 130),
            };

            buf[y as usize][x as usize] = format!("{}✦{}", rgb(col.0, col.1, col.2), RESET);
        }
    }

    fn paint_animal(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, animal: Animal) {
        let x = animal.x.round() as i32;
        let y = animal.y.round() as i32;
        let hop = animal.phase.sin().round() as i32;

        let (body, head, color) = match animal.kind {
            AnimalKind::Fox => ('◆', '›', (210, 95, 35)),
            AnimalKind::Rabbit => ('•', 'ᵔ', (210, 205, 190)),
            AnimalKind::Deer => ('◖', 'ᐧ', (165, 105, 55)),
        };

        let dir = if animal.vx >= 0.0 { 1 } else { -1 };

        if in_bounds(w, h, x, y - hop) {
            buf[(y - hop) as usize][x as usize] = format!("{}{}{}", rgb(color.0, color.1, color.2), body, RESET);
        }

        if in_bounds(w, h, x + dir, y - hop) {
            buf[(y - hop) as usize][(x + dir) as usize] =
                format!("{}{}{}", rgb(color.0, color.1, color.2), head, RESET);
        }
    }

    fn paint_season_indicator(&self, buf: &mut Vec<Vec<String>>, w: usize, h: usize, t_abs: f64) {
        if w < 12 || h < 2 {
            return;
        }

        let (col, count) = match self.season {
            Season::Spring => ((120, 230, 120), 1),
            Season::Summer => ((230, 255, 130), 2),
            Season::Autumn => ((240, 150, 70), 3),
            Season::Winter => ((185, 220, 255), 4),
        };

        for i in 0..count {
            let pulse = ((t_abs * 2.2 + i as f64).sin() + 1.0) * 0.5;
            let c = blend(col, (255, 255, 255), pulse * 0.20);
            let x = 2 + i as i32 * 2;
            if in_bounds(w, h, x, 1) {
                buf[1][x as usize] = format!("{}●{}", rgb(c.0, c.1, c.2), RESET);
            }
        }
    }
}

// ── Low-level helpers ────────────────────────────────────────────────────────

fn in_bounds(w: usize, h: usize, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w as i32 && y < h as i32
}

fn draw_seg(
    buf: &mut Vec<Vec<String>>,
    w: usize,
    h: usize,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    col: (u8, u8, u8),
    ch: char,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let steps = (dx.abs().max(dy.abs()) as usize + 1).max(1);

    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let x = (x1 + dx * t).round() as i32;
        let y = (y1 + dy * t).round() as i32;

        if in_bounds(w, h, x, y) {
            buf[y as usize][x as usize] =
                format!("{}{}{}", rgb(col.0, col.1, col.2), ch, RESET);
        }
    }
}

fn branch_char(dx: f64, dy: f64, depth: u32, season: Season) -> char {
    if depth >= 7 {
        return match season {
            Season::Spring => '✿',
            Season::Autumn => '*',
            Season::Winter => '·',
            Season::Summer => '*',
        };
    }

    if depth >= 5 {
        return '·';
    }

    let adx = dx.abs();
    let ady = dy.abs();

    if ady > adx * 2.0 {
        '│'
    } else if adx > ady * 2.0 {
        '─'
    } else if (dx > 0.0) == (dy < 0.0) {
        '/'
    } else {
        '\\'
    }
}

fn branch_color(depth: u32, season: Season, mode: ColorMode, shift: f64) -> (u8, u8, u8) {
    let t = (depth as f64 / 8.0).clamp(0.0, 1.0);

    let trunk = match mode {
        ColorMode::Ocean => (58, 58, 50),
        ColorMode::Sunset => (100, 50, 22),
        ColorMode::Matrix => (38, 70, 35),
        ColorMode::Rainbow => (82, 52, 22),
    };

    let leaf = leaf_color(season, mode, shift, t);
    lerp_rgb(trunk, leaf, t.powf(1.35))
}

fn leaf_color(season: Season, mode: ColorMode, shift: f64, amount: f64) -> (u8, u8, u8) {
    let base = match season {
        Season::Spring => (85, 220, 95),
        Season::Summer => (35, 180, 65),
        Season::Autumn => {
            let warm = ((shift + amount) * TAU).sin() * 0.5 + 0.5;
            lerp_rgb((235, 130, 55), (210, 185, 65), warm)
        }
        Season::Winter => (170, 190, 185),
    };

    match mode {
        ColorMode::Ocean => blend(base, (85, 170, 210), 0.16),
        ColorMode::Sunset => blend(base, (255, 130, 65), 0.16),
        ColorMode::Matrix => blend(base, (40, 255, 85), 0.20),
        ColorMode::Rainbow => base,
    }
}

fn lerp_rgb(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    let c = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

fn blend(a: (u8, u8, u8), b: (u8, u8, u8), t: f64) -> (u8, u8, u8) {
    lerp_rgb(a, b, t)
}
