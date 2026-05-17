// ===== src/modes/castle_siege.rs =====
//
// Two castles, opposite sides of the screen. Each castle periodically flings
// a random projectile (cannonball / fireball / ice bolt / lightning / boulder)
// in a ballistic arc toward the enemy. On impact, blocks in the target castle
// are destroyed and rubble falls to the ground. Between shots, castles rebuild
// — each redesign uses a new random seed, so the castle keeps "rebuilding
// differently" as the battle goes on.
//
// RENDERING LAYERS:
//   sky gradient → clouds → ground → castles → rubble → projectile trails
//   → projectiles → explosion particles → screen shake offset
//
// PHYSICS:
//   Gravity pulls projectiles down. Launch velocity is solved analytically
//   to hit near the enemy castle center given a chosen flight time.

use crate::ansi::{rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

// ── Block types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum Block { Empty, Wall, Battlement, Window, Door, Pole, Flag }

// ── Ammunition ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Ammo { Cannonball, Fireball, IceBolt, Lightning, Boulder }

impl Ammo {
    fn glyph(self) -> char {
        match self {
            Ammo::Cannonball => '●',
            Ammo::Fireball   => '✦',
            Ammo::IceBolt    => '❋',
            Ammo::Lightning  => '⚡',
            Ammo::Boulder    => '⬤',
        }
    }
    fn color(self) -> Rgb {
        match self {
            Ammo::Cannonball => (130, 130, 140),
            Ammo::Fireball   => (255, 140, 40),
            Ammo::IceBolt    => (140, 220, 255),
            Ammo::Lightning  => (220, 160, 255),
            Ammo::Boulder    => (100, 80, 60),
        }
    }
    /// Particle palette for the explosion this ammo produces.
    fn explosion_palette(self) -> &'static [Rgb] {
        match self {
            Ammo::Cannonball => &[(140, 130, 110), (100, 90, 80), (70, 60, 55)],
            Ammo::Fireball   => &[(255, 220, 80), (255, 120, 30), (200, 40, 20)],
            Ammo::IceBolt    => &[(220, 245, 255), (120, 200, 255), (70, 130, 210)],
            Ammo::Lightning  => &[(255, 230, 255), (220, 180, 255), (180, 100, 240)],
            Ammo::Boulder    => &[(120, 100, 80), (80, 60, 50), (50, 40, 30)],
        }
    }
    /// How much screen-shake magnitude the impact produces.
    fn shake(self) -> f64 {
        match self {
            Ammo::Boulder    => 2.2,
            Ammo::Fireball   => 1.4,
            Ammo::Cannonball => 1.0,
            Ammo::Lightning  => 0.8,
            Ammo::IceBolt    => 0.6,
        }
    }
    /// Damage radius — boulder smashes a 2x2 area, everything else hits 1 block.
    fn blast_radius(self) -> i32 {
        match self { Ammo::Boulder => 1, _ => 0 }
    }
}

// ── Structs ───────────────────────────────────────────────────────────────────

struct Castle {
    base_col:       i32,  // leftmost column on screen
    ground_row:     i32,  // row where the castle's base sits (bottom of blocks)
    grid_w:         usize,
    grid_h:         usize,
    blocks:         Vec<Vec<Block>>,  // current damaged state [row][col]
    template:       Vec<Vec<Block>>,  // what the intact castle looks like
    stone:          Rgb,
    banner:         Rgb,
    faces_right:    bool,
    shoot_timer:    f64,
    rebuild_timer:  f64,
    redesign_timer: f64,
}

struct Projectile {
    x:     f64,
    y:     f64,
    vx:    f64,
    vy:    f64,
    kind:  Ammo,
    owner: usize,
    trail: Vec<(f64, f64)>, // recent positions (newest first)
    age:   f64,
}

struct Particle {
    x: f64, y: f64,
    vx: f64, vy: f64,
    life: f64, max_life: f64,
    col: Rgb,
    ch: char,
    gravity: f64,
}

struct Rubble {
    x: f64, y: f64,
    vx: f64, vy: f64,
    ch: char,
    col: Rgb,
    grounded: bool,
    ground_y: f64,
}

struct Cloud {
    x: f64, y: f64,
    w: f64,
    speed: f64,
}

// ── The mode ──────────────────────────────────────────────────────────────────

pub struct CastleSiegeMode {
    speed: f64,
    _color: ColorProvider,
    castles: Vec<Castle>,          // exactly 2
    projectiles: Vec<Projectile>,
    particles: Vec<Particle>,
    rubble: Vec<Rubble>,
    clouds: Vec<Cloud>,
    flag_time: f64,
    shake_t: f64,
    shake_mag: f64,
    initialized: bool,
}

const GRAVITY: f64 = 22.0;

impl CastleSiegeMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self {
            speed,
            _color: color,
            castles: Vec::new(),
            projectiles: Vec::new(),
            particles: Vec::new(),
            rubble: Vec::new(),
            clouds: Vec::new(),
            flag_time: 0.0,
            shake_t: 0.0,
            shake_mag: 0.0,
            initialized: false,
        }
    }

    fn init(&mut self, width: u16, height: u16) {
        let w = width as i32;
        let h = height as i32;
        let mut rng = rand::rng();

        // Castle dimensions scale with terminal size
        let grid_w     = ((w as usize / 4).clamp(10, 20)).min((w as usize).saturating_sub(6) / 2);
        let grid_h     = (h as usize / 2).clamp(8, 14);
        let ground_row = h - (h / 8).max(2);

        let seed_l = rng.random_range(0..u64::MAX);
        let seed_r = rng.random_range(0..u64::MAX);

        self.castles.push(Castle {
            base_col: 2,
            ground_row,
            grid_w, grid_h,
            template: generate_template(grid_w, grid_h, seed_l, true),
            blocks:   generate_template(grid_w, grid_h, seed_l, true),
            stone:  (145, 100, 90),    // warm reddish stone
            banner: (220, 45, 55),     // red banner
            faces_right: true,
            shoot_timer:    rng.random_range(1.0..3.0),
            rebuild_timer:  rng.random_range(0.8..1.8),
            redesign_timer: rng.random_range(22.0..38.0),
        });

        self.castles.push(Castle {
            base_col: w - grid_w as i32 - 2,
            ground_row,
            grid_w, grid_h,
            template: generate_template(grid_w, grid_h, seed_r, false),
            blocks:   generate_template(grid_w, grid_h, seed_r, false),
            stone:  (100, 115, 140),   // cool bluish stone
            banner: (50, 100, 220),
            faces_right: false,
            shoot_timer:    rng.random_range(1.5..3.5),
            rebuild_timer:  rng.random_range(0.8..1.8),
            redesign_timer: rng.random_range(22.0..38.0),
        });

        // Background clouds
        let cloud_count = (w as usize / 25).max(2);
        self.clouds = (0..cloud_count).map(|_| Cloud {
            x: rng.random_range(0.0..w as f64),
            y: rng.random_range(0.0..(h as f64 * 0.4)),
            w: rng.random_range(6.0..14.0),
            speed: rng.random_range(1.5..4.0),
        }).collect();

        self.initialized = true;
    }
}

// ── Deterministic PRNG for castle generation ──────────────────────────────────

struct Lcg(u64);
impl Lcg {
    fn new(seed: u64) -> Self { Lcg(seed ^ 0x9E3779B97F4A7C15) }
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn range(&mut self, lo: u32, hi: u32) -> u32 { lo + self.next() % (hi - lo + 1).max(1) }
    fn prob(&mut self, p: u32) -> bool { self.next() % 100 < p }
}

// ── Castle template generator ─────────────────────────────────────────────────

fn generate_template(w: usize, h: usize, seed: u64, faces_right: bool) -> Vec<Vec<Block>> {
    let mut r = Lcg::new(seed);
    let mut g = vec![vec![Block::Empty; w]; h];

    // Main wall height (how far up from ground)
    let wall_up = (h / 2 + r.range(0, (h / 4).max(1) as u32) as usize).min(h - 1).max(3);
    let wall_top_row = h - wall_up;

    // Fill the main wall
    for row in wall_top_row..h {
        for col in 0..w {
            g[row][col] = Block::Wall;
        }
    }

    // Battlements along the top of the wall
    if wall_top_row > 0 {
        let pattern = r.range(0, 2);
        let br = wall_top_row - 1;
        for col in 0..w {
            let is_bat = match pattern {
                0 => col % 2 == 0,
                1 => col % 3 != 2,
                _ => col % 2 == 1,
            };
            if is_bat { g[br][col] = Block::Battlement; }
        }
    }

    // Towers: 1-3 of them, scattered across the width
    let num_towers = r.range(1, 3) as usize;
    let tower_extra = r.range(2, ((h / 3) as u32).max(3)) as usize;
    let tower_w = if num_towers == 1 { 3.min(w) } else { 2 };

    let tower_cols: Vec<usize> = match num_towers {
        1 => vec![w / 2 - tower_w / 2],
        2 => vec![1, w.saturating_sub(1 + tower_w)],
        _ => vec![0, w / 2 - tower_w / 2, w.saturating_sub(tower_w)],
    };

    for tc in tower_cols {
        let tower_top = wall_top_row.saturating_sub(tower_extra);
        for row in tower_top..wall_top_row {
            for dc in 0..tower_w {
                let col = tc + dc;
                if col < w {
                    g[row][col] = Block::Wall;
                }
            }
        }
        // Battlements on top of tower
        if tower_top > 0 {
            let tbr = tower_top - 1;
            for dc in 0..tower_w {
                let col = tc + dc;
                if col < w && dc % 2 == 0 {
                    g[tbr][col] = Block::Battlement;
                }
            }
        }
    }

    // Scatter some windows in the walls
    let num_windows = r.range(2, 5) as usize;
    for _ in 0..num_windows {
        if h > wall_top_row + 2 {
            let wr = wall_top_row + 1 + r.range(0, (h - wall_top_row - 2).max(1) as u32) as usize;
            let wc = r.range(0, (w - 1) as u32) as usize;
            if wr < h && wc < w && g[wr][wc] == Block::Wall {
                g[wr][wc] = Block::Window;
            }
        }
    }

    // Door placement — facing the enemy side
    let door_c = if faces_right {
        w.saturating_sub(3).min(w - 1)
    } else {
        2.min(w.saturating_sub(1))
    };
    if h >= 2 {
        g[h - 1][door_c] = Block::Door;
        if h >= 2 && r.prob(70) {
            g[h - 2][door_c] = Block::Door;
        }
    }

    // Flag on the tallest point
    let mut highest: Option<(usize, usize)> = None;
    for row in 0..h {
        for col in 0..w {
            if matches!(g[row][col], Block::Wall | Block::Battlement) {
                match highest {
                    None => highest = Some((row, col)),
                    Some((rr, _)) if row < rr => highest = Some((row, col)),
                    _ => {}
                }
            }
        }
    }
    if let Some((hr, hc)) = highest {
        if hr >= 2 {
            g[hr - 1][hc] = Block::Pole;
            g[hr - 2][hc] = Block::Flag;
        } else if hr >= 1 {
            g[hr - 1][hc] = Block::Flag;
        }
    }

    g
}

// ── Castle → world coordinate helpers ─────────────────────────────────────────

impl Castle {
    /// World (col, row) of the top-left corner of the castle's block grid.
    fn top_left(&self) -> (i32, i32) {
        (self.base_col, self.ground_row - self.grid_h as i32 + 1)
    }
    fn center_col(&self) -> f64 { self.base_col as f64 + self.grid_w as f64 / 2.0 }
    fn center_row(&self) -> f64 { self.ground_row as f64 - self.grid_h as f64 / 2.0 }
    fn top_row(&self)    -> f64 { (self.ground_row - self.grid_h as i32 + 1) as f64 }
}

// ── Physics helpers ───────────────────────────────────────────────────────────

fn launch(from: &Castle, to: &Castle, rng: &mut impl RngExt, speed: f64) -> Projectile {
    // Launch from the top-center of the firing castle
    let x0 = from.center_col();
    let y0 = from.top_row() - 0.5;

    // Aim at the enemy castle center with spread
    let tx = to.center_col()   + rng.random_range(-4.0..4.0);
    let ty = to.center_row()   + rng.random_range(-2.0..3.0);

    // Flight time controls arc height. Solve for velocity.
    let t = rng.random_range(1.7..2.7) / speed.max(0.4);
    let vx = (tx - x0) / t;
    let vy = (ty - y0) / t - 0.5 * GRAVITY * t;

    let kind = match rng.random_range(0..5) {
        0 => Ammo::Cannonball,
        1 => Ammo::Fireball,
        2 => Ammo::IceBolt,
        3 => Ammo::Lightning,
        _ => Ammo::Boulder,
    };

    Projectile {
        x: x0, y: y0,
        vx, vy,
        kind,
        owner: if from.faces_right { 0 } else { 1 },
        trail: Vec::with_capacity(16),
        age: 0.0,
    }
}

/// If the projectile is overlapping a non-empty block in the given castle,
/// return (row, col) of the block it hit.
fn hit_block(p: &Projectile, c: &Castle) -> Option<(usize, usize)> {
    let (tlc, tlr) = c.top_left();
    let rc = (p.x.floor() as i32) - tlc;
    let rr = (p.y.floor() as i32) - tlr;
    if rc < 0 || rr < 0 || rc >= c.grid_w as i32 || rr >= c.grid_h as i32 {
        return None;
    }
    let (rr, rc) = (rr as usize, rc as usize);
    if c.blocks[rr][rc] != Block::Empty {
        Some((rr, rc))
    } else {
        None
    }
}

// ── Main mode impl ────────────────────────────────────────────────────────────

impl Mode for CastleSiegeMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if !self.initialized { self.init(width, height); return; }
        let dt = dt * self.speed;
        self.flag_time += dt;
        let mut rng = rand::rng();

        // ── Clouds ────────────────────────────────────────────────────────
        for c in &mut self.clouds {
            c.x += c.speed * dt;
            if c.x > width as f64 + c.w { c.x = -c.w; }
        }

        // ── Shake decay ───────────────────────────────────────────────────
        if self.shake_t > 0.0 {
            self.shake_t -= dt;
            if self.shake_t <= 0.0 { self.shake_mag = 0.0; }
        }

        // ── Castle timers: shoot / rebuild / redesign ─────────────────────
        // We pre-collect the "shoot" events so we don't have aliasing issues
        // when a castle needs to launch a projectile that depends on the other.
        let mut shots: Vec<usize> = Vec::new();
        for (i, c) in self.castles.iter_mut().enumerate() {
            c.shoot_timer   -= dt;
            c.rebuild_timer -= dt;
            c.redesign_timer -= dt;
            if c.shoot_timer <= 0.0 {
                shots.push(i);
                c.shoot_timer = rng.random_range(2.0..4.5) / self.speed.max(0.4);
            }
            if c.rebuild_timer <= 0.0 {
                c.rebuild_timer = rng.random_range(0.6..1.4) / self.speed.max(0.4);
                // Restore one mismatched block toward template
                let mut candidates = Vec::new();
                for r in 0..c.grid_h {
                    for col in 0..c.grid_w {
                        if c.blocks[r][col] == Block::Empty && c.template[r][col] != Block::Empty {
                            candidates.push((r, col));
                        }
                    }
                }
                if !candidates.is_empty() {
                    let (r, col) = candidates[rng.random_range(0..candidates.len())];
                    c.blocks[r][col] = c.template[r][col];
                }
            }
            if c.redesign_timer <= 0.0 {
                // Regenerate the template: castle will now rebuild toward a new design
                let new_seed = rng.random_range(0..u64::MAX);
                c.template = generate_template(c.grid_w, c.grid_h, new_seed, c.faces_right);
                c.redesign_timer = rng.random_range(20.0..35.0);
            }
        }

        // Fire the queued shots
        for shooter_i in shots {
            let target_i = 1 - shooter_i;
            // Clone-like access through index to avoid borrow issues
            let (shooter_cc, shooter_tr, shooter_faces) = {
                let c = &self.castles[shooter_i];
                (c.center_col(), c.top_row(), c.faces_right)
            };
            let _ = (shooter_cc, shooter_tr, shooter_faces);
            // Build a projectile using both castles read-only
            let shooter = &self.castles[shooter_i];
            let target  = &self.castles[target_i];
            self.projectiles.push(launch(shooter, target, &mut rng, self.speed));
        }

        // ── Projectile physics ────────────────────────────────────────────
        let w = width as f64;
        let h = height as f64;

        let mut new_projectiles: Vec<Projectile> = Vec::with_capacity(self.projectiles.len());
        let mut pending_hits: Vec<(usize, usize, usize, Ammo, f64, f64)> =
            Vec::new(); // (castle_idx, row, col, ammo, x, y)

        // Take ownership to iterate and move into new_projectiles
        let mut current = std::mem::take(&mut self.projectiles);
        for mut p in current.drain(..) {
            // Record trail
            p.trail.insert(0, (p.x, p.y));
            if p.trail.len() > 12 { p.trail.truncate(12); }

            // Integrate
            p.age += dt;
            p.vy  += GRAVITY * dt;
            p.x   += p.vx * dt;
            p.y   += p.vy * dt;

            // Out of bounds or hit ground
            if p.y >= self.castles[0].ground_row as f64 + 1.0
                || p.x < -5.0 || p.x > w + 5.0 || p.age > 8.0 {
                // Ground impact: small dust burst
                if p.y >= self.castles[0].ground_row as f64 - 0.5 {
                    for pal in p.kind.explosion_palette() {
                        for _ in 0..3 {
                            let ang = rng.random_range(0.0..std::f64::consts::TAU);
                            let spd = rng.random_range(2.0..7.0);
                            self.particles.push(Particle {
                                x: p.x, y: p.y,
                                vx: ang.cos() * spd,
                                vy: -(spd * ang.sin().abs()) * 0.6,
                                life: rng.random_range(0.3..0.7),
                                max_life: 0.7,
                                col: *pal,
                                ch: ['*', '·', '°', '˙'][rng.random_range(0..4)],
                                gravity: 15.0,
                            });
                        }
                    }
                }
                continue; // don't keep this projectile
            }

            // Hit test against the enemy castle (skip owner's own)
            let mut hit_found = None;
            for (ci, c) in self.castles.iter().enumerate() {
                if ci == p.owner { continue; }
                if let Some((r, col)) = hit_block(&p, c) {
                    hit_found = Some((ci, r, col));
                    break;
                }
            }

            if let Some((ci, r, col)) = hit_found {
                pending_hits.push((ci, r, col, p.kind, p.x, p.y));
                continue;
            }
            new_projectiles.push(p);
        }
        self.projectiles = new_projectiles;

        // ── Apply damage from hits ────────────────────────────────────────
        for (ci, r, c, ammo, px, py) in pending_hits {
            // Blast radius: damage the center block, plus surrounding if boulder
            let rad = ammo.blast_radius();
            let castle_tl = self.castles[ci].top_left();
            for dr in -rad..=rad {
                for dc in -rad..=rad {
                    let nr = r as i32 + dr;
                    let nc = c as i32 + dc;
                    if nr < 0 || nc < 0
                        || nr >= self.castles[ci].grid_h as i32
                        || nc >= self.castles[ci].grid_w as i32 { continue; }
                    let (nr, nc) = (nr as usize, nc as usize);
                    let block = self.castles[ci].blocks[nr][nc];
                    if block == Block::Empty { continue; }

                    let stone = self.castles[ci].stone;
                    let banner = self.castles[ci].banner;
                    let (ch, col) = match block {
                        Block::Wall | Block::Battlement => ('▓', stone),
                        Block::Window                   => ('·', (40, 35, 50)),
                        Block::Door                     => ('▓', (105, 70, 40)),
                        Block::Pole                     => ('│', (90, 60, 30)),
                        Block::Flag                     => ('▀', banner),
                        _                               => ('.', (100, 100, 100)),
                    };

                    // Spawn rubble from the destroyed block
                    let wx = (castle_tl.0 + nc as i32) as f64;
                    let wy = (castle_tl.1 + nr as i32) as f64;
                    self.rubble.push(Rubble {
                        x: wx, y: wy,
                        vx: rng.random_range(-4.0..4.0),
                        vy: rng.random_range(-8.0..-2.0),
                        ch, col,
                        grounded: false,
                        ground_y: self.castles[ci].ground_row as f64,
                    });

                    self.castles[ci].blocks[nr][nc] = Block::Empty;
                }
            }

            // Explosion particles
            for pal in ammo.explosion_palette() {
                for _ in 0..6 {
                    let ang = rng.random_range(0.0..std::f64::consts::TAU);
                    let spd = rng.random_range(3.0..10.0);
                    self.particles.push(Particle {
                        x: px, y: py,
                        vx: ang.cos() * spd,
                        vy: ang.sin() * spd * 0.6,
                        life: rng.random_range(0.25..0.6),
                        max_life: 0.6,
                        col: *pal,
                        ch: ['*','+','×','✦','·','◦'][rng.random_range(0..6)],
                        gravity: 10.0,
                    });
                }
            }

            // // Screen shake
            // self.shake_mag = self.shake_mag.max(ammo.shake());
            // self.shake_t   = self.shake_t.max(0.35);
        }

        // ── Particles ─────────────────────────────────────────────────────
        for p in &mut self.particles {
            p.vy   += p.gravity * dt;
            p.x    += p.vx * dt;
            p.y    += p.vy * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0 && p.y < h + 2.0);

        // ── Rubble ────────────────────────────────────────────────────────
        for r in &mut self.rubble {
            if r.grounded { continue; }
            r.vy += GRAVITY * 0.6 * dt;
            r.x  += r.vx * dt;
            r.y  += r.vy * dt;
            if r.y >= r.ground_y {
                r.y = r.ground_y;
                r.vx *= 0.0;
                r.vy = 0.0;
                r.grounded = true;
            }
        }
        // Keep a cap on rubble so it doesn't grow unbounded
        if self.rubble.len() > 250 {
            let drop = self.rubble.len() - 250;
            self.rubble.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, _t_abs: f64) -> String {
        let w = width  as usize;
        let h = height as usize;
        let mut buf: Vec<Vec<String>> = vec![vec![String::from(" "); w]; h];

        // Screen shake offset
        let (sx, sy) = if self.shake_t > 0.0 {
            let mut rng = rand::rng();
            let m = self.shake_mag * (self.shake_t / 0.35).clamp(0.0, 1.0);
            (
                rng.random_range(-m..=m).round() as i32,
                rng.random_range(-(m * 0.5)..=m * 0.5).round() as i32,
            )
        } else { (0, 0) };

        let plot = |buf: &mut Vec<Vec<String>>, x: i32, y: i32, col: Rgb, ch: char| {
            let xx = x + sx;
            let yy = y + sy;
            if xx >= 0 && xx < w as i32 && yy >= 0 && yy < h as i32 {
                buf[yy as usize][xx as usize] = format!("{}{}{}", rgb(col.0, col.1, col.2), ch, RESET);
            }
        };

        // ── Sky gradient ──────────────────────────────────────────────────
        let ground = if self.castles.is_empty() { h as i32 - 2 } else { self.castles[0].ground_row };
        for y in 0..ground.max(0) {
            let t  = y as f64 / ground.max(1) as f64;
            let r  = (110.0 + t * 90.0) as u8;
            let g  = (155.0 + t * 65.0) as u8;
            let b  = (215.0 - t * 10.0) as u8;
            for x in 0..w {
                buf[y as usize][x] = format!("{} {}", rgb(r, g, b), RESET);
            }
        }

        // ── Clouds ────────────────────────────────────────────────────────
        for c in &self.clouds {
            let cy = c.y as i32;
            let cx = c.x as i32;
            for dx in 0..(c.w as i32) {
                for dy in 0i32..=1 {
                    let px = cx + dx;
                    let py = cy + dy;
                    let edge = (dx == 0 || dx == c.w as i32 - 1) as i32;
                    let shade = 235u8 - (edge as u8 * 20);
                    if px >= 0 && px < w as i32 && py >= 0 && py < ground {
                        buf[py as usize][px as usize] =
                            format!("{}{}{}", rgb(shade, shade, shade.saturating_add(8)),
                                    if dy == 0 { '▀' } else { '▄' }, RESET);
                    }
                }
            }
        }

        // ── Ground ────────────────────────────────────────────────────────
        for y in ground..h as i32 {
            let depth = (y - ground) as f64 / (h as i32 - ground).max(1) as f64;
            let r = (48.0  - depth * 15.0) as u8;
            let g = (80.0  - depth * 25.0) as u8;
            let b = (38.0  - depth * 10.0) as u8;
            for x in 0..w {
                buf[y as usize][x] = format!("{}▓{}", rgb(r, g, b), RESET);
            }
        }

        // ── Castles ───────────────────────────────────────────────────────
        for c in &self.castles {
            let (tlc, tlr) = c.top_left();
            for r in 0..c.grid_h {
                for col in 0..c.grid_w {
                    let block = c.blocks[r][col];
                    if block == Block::Empty { continue; }
                    let x = tlc + col as i32;
                    let y = tlr + r as i32;
                    let (ch, color) = match block {
                        Block::Wall => {
                            // Subtle shading: edges darker than middle
                            let is_edge = col == 0 || col == c.grid_w - 1
                                       || r == c.grid_h - 1;
                            let s = c.stone;
                            let shade = if is_edge { (s.0 as i32 * 80 / 100,
                                                      s.1 as i32 * 80 / 100,
                                                      s.2 as i32 * 80 / 100) }
                                        else { (s.0 as i32, s.1 as i32, s.2 as i32) };
                            ('█', (shade.0 as u8, shade.1 as u8, shade.2 as u8))
                        }
                        Block::Battlement => ('▀', c.stone),
                        Block::Window     => ('▪', (30, 30, 55)),
                        Block::Door       => ('▓', (95, 65, 35)),
                        Block::Pole       => ('│', (70, 50, 30)),
                        Block::Flag => {
                            // Simple wave animation
                            let wave = (self.flag_time * 4.0 + col as f64 * 0.7).sin();
                            let ch = if wave > 0.0 {
                                if c.faces_right { '▶' } else { '◀' }
                            } else {
                                if c.faces_right { '►' } else { '◄' }
                            };
                            (ch, c.banner)
                        }
                        Block::Empty => continue,
                    };
                    plot(&mut buf, x, y, color, ch);
                }
            }
        }

        // ── Rubble on ground ──────────────────────────────────────────────
        for r in &self.rubble {
            let x = r.x.round() as i32;
            let y = r.y.round() as i32;
            plot(&mut buf, x, y, r.col, r.ch);
        }

        // ── Projectile trails ─────────────────────────────────────────────
        for p in &self.projectiles {
            for (i, &(tx, ty)) in p.trail.iter().enumerate().skip(1) {
                let fade = 1.0 - i as f64 / p.trail.len() as f64;
                let (r, g, b) = p.kind.color();
                let col = (
                    (r as f64 * fade * 0.7) as u8,
                    (g as f64 * fade * 0.7) as u8,
                    (b as f64 * fade * 0.7) as u8,
                );
                let ch = if i < 3 { '*' } else { '·' };
                plot(&mut buf, tx.round() as i32, ty.round() as i32, col, ch);
            }
        }

        // ── Particles ─────────────────────────────────────────────────────
        for p in &self.particles {
            let fade = (p.life / p.max_life).clamp(0.0, 1.0);
            let col = (
                (p.col.0 as f64 * fade) as u8,
                (p.col.1 as f64 * fade) as u8,
                (p.col.2 as f64 * fade) as u8,
            );
            plot(&mut buf, p.x.round() as i32, p.y.round() as i32, col, p.ch);
        }

        // ── Projectiles (on top of everything) ────────────────────────────
        for p in &self.projectiles {
            plot(&mut buf, p.x.round() as i32, p.y.round() as i32, p.kind.color(), p.kind.glyph());
        }

        buf.into_iter().map(|r| r.join("")).collect::<Vec<_>>().join("\n")
    }
}
