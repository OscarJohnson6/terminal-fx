// ===================================================================
//  src/modes/ant_colony.rs
// -------------------------------------------------------------------
//  AntColonyMode v2
//
//  A more stable rewrite of the organic ant-colony mode.
//
//  The previous version had a cool concept, but it was easy for behavior
//  to get weird because ants, food, rooms, and grid mutation were all
//  tangled together. This version uses a cleaner update model:
//
//    1. Take a lightweight grid/food snapshot.
//    2. Let every ant decide what it wants to do.
//    3. Store those decisions in local action vectors.
//    4. Apply digging, rooms, food pickup, food delivery, births, deaths.
//
//  Result:
//    - Diggers keep carving instead of stalling.
//    - Rooms appear more reliably.
//    - Foragers can actually find and remove food safely.
//    - Queen lifecycle is less fragile.
//    - Colony has more visible phases and recovery.
//    - Surface events remain, but do not randomly break the whole colony.
//
//  Suggested registry:
//    id:   "ant_colony"
//    name: "Ant Colony"
//    desc: "Organic digging colony with queen lifecycle"
//    fps:  50
// ===================================================================

use crate::ansi::RESET;
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

// ── Cells ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Sky,
    Grass,
    Entrance,
    Dirt,
    Tunnel,
    RoomFood,
    RoomQueen,
    RoomNursery,
}

impl Cell {
    fn is_walkable(self) -> bool {
        matches!(
            self,
            Cell::Entrance | Cell::Tunnel | Cell::RoomFood | Cell::RoomQueen | Cell::RoomNursery
        )
    }

    fn is_underground_room(self) -> bool {
        matches!(self, Cell::RoomFood | Cell::RoomQueen | Cell::RoomNursery)
    }
}

// ── Ant jobs ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Job {
    Wander,
    Dig,
    ForageUp,
    ForageSurface,
    CarryFood,
    Queen,
    Larva { timer: f32 },
}

struct Ant {
    x: f32,
    y: f32,
    step_timer: f32,
    job: Job,
    alive: bool,
    food_carried: bool,
    energy: f32,
}

struct FoodItem {
    x: usize,
    y: usize,
    age: f32,
}

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    col: Rgb,
}

#[derive(Clone, Copy)]
enum SurfaceEvt {
    None,
    Stomp { cx: usize, radius: f32, age: f32 },
    Bird { x: f32, going_right: bool, age: f32 },
}

#[derive(Clone, Copy)]
struct ExcavReq {
    cx: usize,
    cy: usize,
    room: Cell,
}

#[derive(Clone, Copy)]
struct DigReq {
    x: usize,
    y: usize,
}

#[derive(Clone, Copy)]
struct FoodPickup {
    ant_idx: usize,
    food_idx: usize,
}

#[derive(Clone, Copy)]
struct FoodDelivery {
    ant_idx: usize,
}

#[derive(Clone, Copy)]
struct KillReq {
    ant_idx: usize,
}

const MAX_ANTS: usize = 70;
const INITIAL_ANTS: usize = 18;

const QUEEN_DRAIN: f32 = 0.0065;
const QUEEN_GAIN: f32 = 0.24;
const LARVA_HATCH: f32 = 12.0;
const SUCCESSION_SECS: f32 = 9.0;

const FOOD_ROOM_DEPTH: f64 = 0.22;
const QUEEN_ROOM_DEPTH: f64 = 0.45;
const NURSERY_DEPTH: f64 = 0.62;

pub struct AntColonyMode {
    speed: f64,
    _color: ColorProvider,

    grid: Vec<Vec<Cell>>,
    grid_w: usize,
    grid_h: usize,
    ground_y: usize,

    ants: Vec<Ant>,
    food_items: Vec<FoodItem>,
    sparks: Vec<Spark>,

    surface_evt: SurfaceEvt,

    food_stored: u32,
    queen_alive: bool,
    queen_health: f32,
    queen_pos: (usize, usize),

    food_room_pos: Option<(usize, usize)>,
    queen_room_pos: Option<(usize, usize)>,
    nursery_pos: Option<(usize, usize)>,

    succession_active: bool,
    succession_timer: f32,

    food_spawn_timer: f64,
    event_timer: f64,
    next_event: f64,
    queen_lay_timer: f64,
    maintenance_timer: f64,
    colony_age: f64,

    noise_seed: u64,
    initialized: bool,
}

impl AntColonyMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self {
            speed,
            _color: color,
            grid: Vec::new(),
            grid_w: 0,
            grid_h: 0,
            ground_y: 0,
            ants: Vec::new(),
            food_items: Vec::new(),
            sparks: Vec::new(),
            surface_evt: SurfaceEvt::None,
            food_stored: 0,
            queen_alive: false,
            queen_health: 1.0,
            queen_pos: (0, 0),
            food_room_pos: None,
            queen_room_pos: None,
            nursery_pos: None,
            succession_active: false,
            succession_timer: 0.0,
            food_spawn_timer: 4.0,
            event_timer: 0.0,
            next_event: 38.0,
            queen_lay_timer: 18.0,
            maintenance_timer: 0.0,
            colony_age: 0.0,
            noise_seed: 0,
            initialized: false,
        }
    }

    fn init(&mut self, w: u16, h: u16) {
        let mut rng = rand::rng();

        self.grid_w = (w as usize).max(20);
        self.grid_h = (h as usize).max(14);
        self.ground_y = (self.grid_h / 7).max(3).min(self.grid_h.saturating_sub(5));
        self.noise_seed = rng.random_range(0..u64::MAX);

        self.grid = (0..self.grid_h)
            .map(|y| {
                (0..self.grid_w)
                    .map(|_| {
                        if y < self.ground_y {
                            Cell::Sky
                        } else if y == self.ground_y {
                            Cell::Grass
                        } else {
                            Cell::Dirt
                        }
                    })
                    .collect()
            })
            .collect();

        let cx = self.grid_w / 2;

        for dx in 0..3 {
            let x = cx.saturating_sub(1) + dx;
            if x < self.grid_w {
                self.grid[self.ground_y][x] = Cell::Entrance;
            }
        }

        // Starter shaft and tiny hub. After this, ants do most of the excavation.
        let starter_depth = ((self.grid_h - self.ground_y) as f32 * 0.20).max(4.0) as usize;
        for dy in 1..=starter_depth {
            let y = (self.ground_y + dy).min(self.grid_h - 1);
            for dx in -1i32..=1 {
                let x = (cx as i32 + dx).clamp(1, self.grid_w as i32 - 2) as usize;
                self.grid[y][x] = Cell::Tunnel;
            }
        }

        let hub_y = (self.ground_y + starter_depth).min(self.grid_h - 2);
        self.carve_oval(cx, hub_y, 6.5, 2.5, Cell::Tunnel);

        self.ants.clear();
        for i in 0..INITIAL_ANTS {
            self.ants.push(Ant {
                x: cx as f32 + rng.random_range(-2.0..2.0),
                y: (self.ground_y + 2) as f32,
                step_timer: rng.random_range(0.0..0.25),
                job: if i < 8 { Job::Dig } else { Job::Wander },
                alive: true,
                food_carried: false,
                energy: rng.random_range(0.7..1.0),
            });
        }

        self.food_items.clear();
        self.sparks.clear();
        self.surface_evt = SurfaceEvt::None;

        self.food_stored = 0;
        self.queen_alive = false;
        self.queen_health = 1.0;
        self.queen_pos = (cx, hub_y);
        self.food_room_pos = None;
        self.queen_room_pos = None;
        self.nursery_pos = None;
        self.succession_active = false;
        self.succession_timer = 0.0;

        self.food_spawn_timer = 2.5;
        self.event_timer = 0.0;
        self.next_event = rng.random_range(32.0..58.0) / self.speed.max(0.5);
        self.queen_lay_timer = 18.0;
        self.maintenance_timer = 0.0;
        self.colony_age = 0.0;
        self.initialized = true;
    }

    fn carve_oval(&mut self, cx: usize, cy: usize, rw: f32, rh: f32, cell: Cell) {
        let seed = self.noise_seed;

        for dy in -(rh as i32 + 2)..=(rh as i32 + 2) {
            for dx in -(rw as i32 + 2)..=(rw as i32 + 2) {
                let x = (cx as i32 + dx).clamp(1, self.grid_w as i32 - 2) as usize;
                let y = (cy as i32 + dy).clamp(self.ground_y as i32 + 1, self.grid_h as i32 - 1) as usize;

                let wobble = stable_noise(x, y, seed) as f32 * 0.32;
                let ellipse = (dx as f32 / rw).powi(2) + (dy as f32 / rh).powi(2);

                if ellipse <= 1.0 + wobble {
                    self.grid[y][x] = cell;
                }
            }
        }
    }

    fn excavate_room(&mut self, req: ExcavReq) {
        let (rw, rh, color) = match req.room {
            Cell::RoomFood => (8.5, 3.2, (215, 135, 45)),
            Cell::RoomQueen => (11.5, 4.8, (255, 210, 70)),
            Cell::RoomNursery => (9.0, 3.5, (90, 150, 235)),
            _ => (6.0, 2.8, (160, 140, 110)),
        };

        self.carve_oval(req.cx, req.cy, rw, rh, req.room);

        // Always connect room center back into the tunnel network.
        self.grid[req.cy][req.cx] = req.room;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let x = (req.cx as i32 + dx).clamp(1, self.grid_w as i32 - 2) as usize;
                let y = (req.cy as i32 + dy).clamp(self.ground_y as i32 + 1, self.grid_h as i32 - 1) as usize;
                if self.grid[y][x] == Cell::Dirt {
                    self.grid[y][x] = Cell::Tunnel;
                }
            }
        }

        self.spark_burst(req.cx as f32, req.cy as f32, color, 20);
    }

    fn spark_burst(&mut self, x: f32, y: f32, color: Rgb, count: usize) {
        let mut rng = rand::rng();

        for _ in 0..count {
            let a: f32 = rng.random_range(0.0..std::f32::consts::TAU);
            let s: f32 = rng.random_range(2.0..8.0);

            self.sparks.push(Spark {
                x,
                y,
                vx: a.cos() * s,
                vy: a.sin() * s * 0.55,
                life: rng.random_range(0.35..1.0),
                max_life: 1.0,
                col: color,
            });
        }
    }

    fn ensure_colony_progress(&mut self) {
        // Safety valve: if ants fail to organically create rooms in time,
        // create room requests near the current deepest tunnel. This keeps
        // the mode visually alive instead of silently waiting forever.
        let deepest = self.deepest_open_cell();

        let Some((dx, dy)) = deepest else {
            return;
        };

        let depth = self.depth_frac(dy);

        if self.food_room_pos.is_none() && (depth > FOOD_ROOM_DEPTH || self.colony_age > 18.0) {
            let req = ExcavReq {
                cx: dx,
                cy: dy,
                room: Cell::RoomFood,
            };
            self.food_room_pos = Some((dx, dy));
            self.excavate_room(req);
            return;
        }

        if self.queen_room_pos.is_none()
            && self.food_room_pos.is_some()
            && (depth > QUEEN_ROOM_DEPTH || self.colony_age > 32.0)
        {
            let qx = (dx as i32 + 6).clamp(2, self.grid_w as i32 - 3) as usize;
            let qy = (dy + 3).min(self.grid_h - 2);
            let req = ExcavReq {
                cx: qx,
                cy: qy,
                room: Cell::RoomQueen,
            };

            self.queen_room_pos = Some((qx, qy));
            self.queen_pos = (qx, qy);
            self.queen_alive = true;
            self.queen_health = 1.0;
            self.excavate_room(req);

            self.ants.push(Ant {
                x: qx as f32,
                y: qy as f32,
                step_timer: 0.0,
                job: Job::Queen,
                alive: true,
                food_carried: false,
                energy: 1.0,
            });
            return;
        }

        if self.nursery_pos.is_none()
            && self.queen_room_pos.is_some()
            && (depth > NURSERY_DEPTH || self.colony_age > 46.0)
        {
            let nx = (dx as i32 - 7).clamp(2, self.grid_w as i32 - 3) as usize;
            let ny = (dy + 4).min(self.grid_h - 2);
            let req = ExcavReq {
                cx: nx,
                cy: ny,
                room: Cell::RoomNursery,
            };

            self.nursery_pos = Some((nx, ny));
            self.excavate_room(req);
        }
    }

    fn deepest_open_cell(&self) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize)> = None;

        for y in self.ground_y + 1..self.grid_h {
            for x in 1..self.grid_w.saturating_sub(1) {
                if self.grid[y][x].is_walkable() {
                    if best.map(|(_, by)| y > by).unwrap_or(true) {
                        best = Some((x, y));
                    }
                }
            }
        }

        best
    }

    fn depth_frac(&self, y: usize) -> f64 {
        let total = self.grid_h.saturating_sub(self.ground_y + 1).max(1) as f64;
        y.saturating_sub(self.ground_y) as f64 / total
    }

    fn spawn_food(&mut self) {
        let mut rng = rand::rng();
        if self.food_items.len() >= 18 {
            return;
        }

        let cluster_x = rng.random_range(self.grid_w / 8..self.grid_w * 7 / 8);
        let count = rng.random_range(1..=3);

        for _ in 0..count {
            let x = (cluster_x as i32 + rng.random_range(-4..=4))
                .clamp(1, self.grid_w as i32 - 2) as usize;
            self.food_items.push(FoodItem {
                x,
                y: self.ground_y,
                age: 0.0,
            });
        }
    }

    fn trigger_surface_event(&mut self) {
        let mut rng = rand::rng();

        if rng.random_bool(0.55) {
            let cx = rng.random_range(self.grid_w / 6..self.grid_w * 5 / 6);
            self.surface_evt = SurfaceEvt::Stomp {
                cx,
                radius: 0.0,
                age: 0.0,
            };

            for _ in 0..rng.random_range(3..7) {
                let x = (cx as i32 + rng.random_range(-5..=5))
                    .clamp(1, self.grid_w as i32 - 2) as usize;
                self.food_items.push(FoodItem {
                    x,
                    y: self.ground_y,
                    age: 0.0,
                });
            }
        } else {
            let going_right = rng.random_bool(0.5);
            let start = if going_right {
                -4.0
            } else {
                self.grid_w as f32 + 4.0
            };

            self.surface_evt = SurfaceEvt::Bird {
                x: start,
                going_right,
                age: 0.0,
            };
        }
    }
}

// ── Update helpers ────────────────────────────────────────────────────────────

fn weighted_pick(candidates: &[((usize, usize), i32)], rng: &mut impl RngExt) -> Option<(usize, usize)> {
    let total: i32 = candidates.iter().map(|c| c.1).sum();

    if total <= 0 {
        return None;
    }

    let mut roll = rng.random_range(0..total);

    for &((x, y), weight) in candidates {
        if roll < weight {
            return Some((x, y));
        }

        roll -= weight;
    }

    candidates.last().map(|c| c.0)
}

fn plan_dig(
    ax: i32,
    ay: i32,
    grid: &[Vec<Cell>],
    gw: usize,
    gh: usize,
    gy: usize,
    rng: &mut impl RngExt,
) -> Option<((usize, usize), bool)> {
    const DIRS: &[(i32, i32, i32)] = &[
        (0, 1, 52),
        (-1, 1, 22),
        (1, 1, 22),
        (-1, 0, 13),
        (1, 0, 13),
        (0, -1, 4),
        (-1, -1, 2),
        (1, -1, 2),
    ];

    let mut cands = Vec::new();

    for &(dx, dy, base) in DIRS {
        let nx = ax + dx;
        let ny = ay + dy;

        if nx < 1 || ny <= gy as i32 || nx >= gw as i32 - 1 || ny >= gh as i32 {
            continue;
        }

        let cell = grid[ny as usize][nx as usize];

        let mut weight = match cell {
            Cell::Dirt => base,
            c if c.is_walkable() => base * 3,
            _ => 0,
        };

        // Avoid digging straight into existing rooms too much.
        if cell.is_underground_room() {
            weight /= 2;
        }

        if weight > 0 {
            cands.push(((nx as usize, ny as usize), weight));
        }
    }

    let (nx, ny) = weighted_pick(&cands, rng)?;
    Some(((nx, ny), grid[ny][nx] == Cell::Dirt))
}

fn plan_walk(
    ax: i32,
    ay: i32,
    grid: &[Vec<Cell>],
    gw: usize,
    gh: usize,
    gy: usize,
    prefer: Option<(usize, usize)>,
    prefer_up: bool,
    prefer_down: bool,
    rng: &mut impl RngExt,
) -> Option<(usize, usize)> {
    const DIRS: &[(i32, i32, i32)] = &[
        (0, 1, 10),
        (0, -1, 10),
        (1, 0, 8),
        (-1, 0, 8),
        (1, 1, 5),
        (-1, 1, 5),
        (1, -1, 5),
        (-1, -1, 5),
    ];

    let mut cands = Vec::new();

    for &(dx, dy, base) in DIRS {
        let nx = ax + dx;
        let ny = ay + dy;

        if nx < 1 || ny <= gy as i32 || nx >= gw as i32 - 1 || ny >= gh as i32 {
            continue;
        }

        let cell = grid[ny as usize][nx as usize];

        if !cell.is_walkable() {
            continue;
        }

        let mut weight = base;

        if prefer_up && dy < 0 {
            weight += 24;
        }

        if prefer_down && dy > 0 {
            weight += 18;
        }

        if let Some((tx, ty)) = prefer {
            let old_dist = (ax - tx as i32).abs() + (ay - ty as i32).abs();
            let new_dist = (nx - tx as i32).abs() + (ny - ty as i32).abs();

            if new_dist < old_dist {
                weight += 24;
            }

            weight += (28 - new_dist).max(0);
        }

        if cell.is_underground_room() {
            weight += 6;
        }

        cands.push(((nx as usize, ny as usize), weight));
    }

    weighted_pick(&cands, rng)
}

fn nearest_food(food: &[FoodItem], x: usize) -> Option<usize> {
    food.iter()
        .enumerate()
        .min_by_key(|(_, f)| (f.x as i32 - x as i32).abs())
        .map(|(i, _)| i)
}

fn stable_noise(x: usize, y: usize, seed: u64) -> f64 {
    let h = (x.wrapping_mul(2_654_435_761) ^ y.wrapping_mul(2_246_822_519))
        .wrapping_add(seed as usize);
    ((h >> 8) & 0xFFFF) as f64 / 65_535.0
}

fn dirt_rgb(x: usize, y: usize, depth: f64, seed: u64) -> Rgb {
    let n = stable_noise(x, y, seed);
    let var = n * 24.0 - 12.0;
    let r = (78.0 - depth * 33.0 + var).clamp(18.0, 118.0) as u8;
    let g = (50.0 - depth * 21.0 + var * 0.6).clamp(10.0, 78.0) as u8;
    let b = (30.0 - depth * 13.0 + var * 0.3).clamp(6.0, 48.0) as u8;
    (r, g, b)
}

fn lerp_rgb(a: Rgb, b: Rgb, t: f64) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let c = |a: u8, b: u8| (a as f64 + (b as f64 - a as f64) * t) as u8;
    (c(a.0, b.0), c(a.1, b.1), c(a.2, b.2))
}

// ── Mode impl ─────────────────────────────────────────────────────────────────

impl Mode for AntColonyMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        if !self.initialized
            || self.grid_w != width as usize
            || self.grid_h != height as usize
        {
            self.init(width, height);
            return;
        }

        let dt = dt * self.speed;
        let dt_f = dt as f32;

        self.colony_age += dt;
        self.food_spawn_timer -= dt;
        self.event_timer += dt;
        self.maintenance_timer += dt;

        if self.food_spawn_timer <= 0.0 {
            let mut rng = rand::rng();
            self.food_spawn_timer = rng.random_range(3.5..7.5) / self.speed.max(0.5);
            self.spawn_food();
        }

        for food in &mut self.food_items {
            food.age += dt_f;
        }

        self.food_items.retain(|f| f.age < 70.0);

        if self.event_timer >= self.next_event {
            let mut rng = rand::rng();
            self.event_timer = 0.0;
            self.next_event = rng.random_range(34.0..68.0) / self.speed.max(0.5);
            self.trigger_surface_event();
        }

        match &mut self.surface_evt {
            SurfaceEvt::Stomp { age, radius, .. } => {
                *age += dt_f;
                *radius += dt_f * 5.5;
                if *age > 1.4 {
                    self.surface_evt = SurfaceEvt::None;
                }
            }
            SurfaceEvt::Bird { x, going_right, age } => {
                *age += dt_f;
                let spd = self.grid_w as f32 * 0.55 * self.speed as f32;
                *x += if *going_right { spd * dt_f } else { -spd * dt_f };

                if *age > 5.0 || *x < -8.0 || *x > self.grid_w as f32 + 8.0 {
                    self.surface_evt = SurfaceEvt::None;
                }
            }
            SurfaceEvt::None => {}
        }

        // Queen health and reproduction.
        if self.queen_alive {
            let stored_support = (self.food_stored as f32 * 0.0018).min(0.010);
            self.queen_health = (self.queen_health - (QUEEN_DRAIN - stored_support).max(0.001) * dt_f).max(0.0);

            if self.queen_health <= 0.0 {
                self.queen_alive = false;
                self.succession_active = true;
                self.succession_timer = SUCCESSION_SECS;
                self.spark_burst(self.queen_pos.0 as f32, self.queen_pos.1 as f32, (190, 45, 45), 26);
            }

            self.queen_lay_timer -= dt;
            if self.queen_lay_timer <= 0.0 && self.food_stored >= 2 && self.ants.len() < MAX_ANTS {
                let mut rng = rand::rng();
                self.queen_lay_timer = rng.random_range(8.0..15.0) / self.speed.max(0.5);
                self.food_stored = self.food_stored.saturating_sub(2);

                let (lx, ly) = self
                    .nursery_pos
                    .or(self.queen_room_pos)
                    .unwrap_or(self.queen_pos);

                self.ants.push(Ant {
                    x: lx as f32 + rng.random_range(-1.5..1.5),
                    y: ly as f32 + rng.random_range(-0.8..0.8),
                    step_timer: 0.0,
                    job: Job::Larva { timer: LARVA_HATCH },
                    alive: true,
                    food_carried: false,
                    energy: 1.0,
                });
            }
        }

        if self.succession_active {
            self.succession_timer -= dt_f;

            if self.succession_timer <= 0.0 {
                self.succession_active = false;

                if let Some(idx) = self.ants.iter().position(|a| matches!(a.job, Job::Larva { .. })) {
                    let x = self.ants[idx].x;
                    let y = self.ants[idx].y;
                    self.ants[idx].job = Job::Queen;
                    self.queen_alive = true;
                    self.queen_health = 0.45;
                    self.queen_pos = (x as usize, y as usize);
                    self.spark_burst(x, y, (255, 215, 80), 24);
                } else if let Some((qx, qy)) = self.queen_room_pos {
                    self.queen_alive = true;
                    self.queen_health = 0.35;
                    self.queen_pos = (qx, qy);
                    self.ants.push(Ant {
                        x: qx as f32,
                        y: qy as f32,
                        step_timer: 0.0,
                        job: Job::Queen,
                        alive: true,
                        food_carried: false,
                        energy: 1.0,
                    });
                    self.spark_burst(qx as f32, qy as f32, (255, 215, 80), 18);
                }
            }
        }

        if self.maintenance_timer > 1.0 {
            self.maintenance_timer = 0.0;
            self.ensure_colony_progress();

            // If the colony has no diggers, convert a wanderer. This prevents silent stalls.
            if !self.ants.iter().any(|a| a.alive && matches!(a.job, Job::Dig)) {
                if let Some(a) = self.ants.iter_mut().find(|a| a.alive && matches!(a.job, Job::Wander)) {
                    a.job = Job::Dig;
                }
            }

            // If food exists and nobody is foraging, send somebody.
            if self.food_room_pos.is_some()
                && !self.food_items.is_empty()
                && !self.ants.iter().any(|a| a.alive && matches!(a.job, Job::ForageUp | Job::ForageSurface | Job::CarryFood))
            {
                if let Some(a) = self.ants.iter_mut().find(|a| a.alive && matches!(a.job, Job::Wander)) {
                    a.job = Job::ForageUp;
                }
            }
        }

        // Snapshot world state for planning.
        let grid_snapshot = self.grid.clone();
        let food_snapshot: Vec<(usize, usize)> = self.food_items.iter().map(|f| (f.x, f.y)).collect();

        let gw = self.grid_w;
        let gh = self.grid_h;
        let gy = self.ground_y;
        let food_room = self.food_room_pos;
        let has_food_room = self.food_room_pos.is_some();
        let has_queen_room = self.queen_room_pos.is_some();
        let has_nursery = self.nursery_pos.is_some();
        let bird_x = match self.surface_evt {
            SurfaceEvt::Bird { x, .. } => Some(x),
            _ => None,
        };
        let stomp = match self.surface_evt {
            SurfaceEvt::Stomp { cx, radius, .. } => Some((cx, radius)),
            _ => None,
        };

        let mut rng = rand::rng();
        let mut dig_reqs: Vec<DigReq> = Vec::new();
        let mut excav_reqs: Vec<ExcavReq> = Vec::new();
        let mut pickups: Vec<FoodPickup> = Vec::new();
        let mut deliveries: Vec<FoodDelivery> = Vec::new();
        let mut kills: Vec<KillReq> = Vec::new();
        let mut births: Vec<Ant> = Vec::new();

        for i in 0..self.ants.len() {
            if !self.ants[i].alive {
                continue;
            }

            // Larvae tick but do not move.
            if let Job::Larva { timer } = self.ants[i].job {
                let next_timer = timer - dt_f;
                if next_timer <= 0.0 {
                    self.ants[i].job = if rng.random_bool(0.35) { Job::Dig } else { Job::Wander };
                    self.ants[i].energy = 1.0;
                } else {
                    self.ants[i].job = Job::Larva { timer: next_timer };
                }
                continue;
            }

            if self.ants[i].job == Job::Queen {
                continue;
            }

            // Surface threats.
            if let Some((cx, radius)) = stomp {
                if self.ants[i].y <= gy as f32 + 0.5
                    && (self.ants[i].x - cx as f32).abs() <= radius + 2.5
                    && rng.random_bool(0.20)
                {
                    kills.push(KillReq { ant_idx: i });
                    continue;
                }
            }

            if let Some(bx) = bird_x {
                if self.ants[i].y <= gy as f32 + 0.5
                    && (self.ants[i].x - bx).abs() < 2.7
                    && rng.random_bool(0.34)
                {
                    kills.push(KillReq { ant_idx: i });
                    continue;
                }
            }

            self.ants[i].step_timer -= dt_f;
            if self.ants[i].step_timer > 0.0 {
                continue;
            }

            self.ants[i].step_timer = rng.random_range(0.045..0.18);
            self.ants[i].energy = (self.ants[i].energy - rng.random_range(0.0005..0.0025)).max(0.0);

            let ax = self.ants[i].x.round().clamp(1.0, gw as f32 - 2.0) as i32;
            let ay = self.ants[i].y.round().clamp(0.0, gh as f32 - 1.0) as i32;

            if self.ants[i].y <= gy as f32 {
                match self.ants[i].job {
                    Job::ForageSurface => {
                        if let Some(food_idx) = nearest_food_from_snapshot(&food_snapshot, ax as usize) {
                            let fx = food_snapshot[food_idx].0;

                            if (fx as i32 - ax).abs() <= 1 {
                                pickups.push(FoodPickup {
                                    ant_idx: i,
                                    food_idx,
                                });
                                self.ants[i].job = Job::CarryFood;
                                self.ants[i].food_carried = true;
                                self.ants[i].y = gy as f32 + 1.0;
                            } else {
                                let dx = if fx as i32 > ax { 1.0 } else { -1.0 };
                                self.ants[i].x = (self.ants[i].x + dx).clamp(1.0, gw as f32 - 2.0);
                            }
                        } else {
                            self.ants[i].x = (self.ants[i].x + if rng.random_bool(0.5) { 1.0 } else { -1.0 })
                                .clamp(1.0, gw as f32 - 2.0);

                            if rng.random_bool(0.10) {
                                self.ants[i].job = Job::Wander;
                                self.ants[i].y = gy as f32 + 1.0;
                            }
                        }
                    }
                    Job::CarryFood => {
                        self.ants[i].y = gy as f32 + 1.0;
                    }
                    _ => {
                        if rng.random_bool(0.55) {
                            self.ants[i].y = gy as f32 + 1.0;
                            self.ants[i].job = if self.ants[i].food_carried { Job::CarryFood } else { Job::Wander };
                        } else {
                            self.ants[i].x = (self.ants[i].x + if rng.random_bool(0.5) { 1.0 } else { -1.0 })
                                .clamp(1.0, gw as f32 - 2.0);
                        }
                    }
                }

                continue;
            }

            match self.ants[i].job {
                Job::Dig => {
                    if let Some(((nx, ny), is_dig)) = plan_dig(ax, ay, &grid_snapshot, gw, gh, gy, &mut rng) {
                        if is_dig {
                            dig_reqs.push(DigReq { x: nx, y: ny });
                        }

                        self.ants[i].x = nx as f32;
                        self.ants[i].y = ny as f32;

                        let depth = (ny.saturating_sub(gy) as f64) / (gh.saturating_sub(gy).max(1) as f64);

                        if self.food_room_pos.is_none() && depth > FOOD_ROOM_DEPTH {
                            excav_reqs.push(ExcavReq {
                                cx: nx,
                                cy: ny,
                                room: Cell::RoomFood,
                            });
                            self.ants[i].job = Job::Wander;
                        } else if has_food_room && !has_queen_room && depth > QUEEN_ROOM_DEPTH {
                            excav_reqs.push(ExcavReq {
                                cx: nx,
                                cy: ny,
                                room: Cell::RoomQueen,
                            });
                            births.push(Ant {
                                x: nx as f32,
                                y: ny as f32,
                                step_timer: 0.0,
                                job: Job::Queen,
                                alive: true,
                                food_carried: false,
                                energy: 1.0,
                            });
                            self.ants[i].job = Job::Wander;
                        } else if has_queen_room && !has_nursery && depth > NURSERY_DEPTH {
                            excav_reqs.push(ExcavReq {
                                cx: nx,
                                cy: ny,
                                room: Cell::RoomNursery,
                            });
                            self.ants[i].job = Job::Wander;
                        }
                    } else {
                        self.ants[i].job = Job::Wander;
                    }

                    if rng.random_bool(0.003) {
                        self.ants[i].job = Job::Wander;
                    }
                }

                Job::Wander => {
                    if let Some((nx, ny)) = plan_walk(
                        ax,
                        ay,
                        &grid_snapshot,
                        gw,
                        gh,
                        gy,
                        None,
                        false,
                        false,
                        &mut rng,
                    ) {
                        self.ants[i].x = nx as f32;
                        self.ants[i].y = ny as f32;
                    }

                    if has_food_room && !self.food_items.is_empty() && rng.random_bool(0.018) {
                        self.ants[i].job = Job::ForageUp;
                    } else if rng.random_bool(0.009) {
                        self.ants[i].job = Job::Dig;
                    }
                }

                Job::ForageUp => {
                    if let Some((nx, ny)) = plan_walk(
                        ax,
                        ay,
                        &grid_snapshot,
                        gw,
                        gh,
                        gy,
                        Some((gw / 2, gy + 1)),
                        true,
                        false,
                        &mut rng,
                    ) {
                        self.ants[i].x = nx as f32;
                        self.ants[i].y = ny as f32;
                    } else {
                        self.ants[i].y = (self.ants[i].y - 1.0).max(gy as f32);
                    }

                    if self.ants[i].y <= gy as f32 + 1.0 {
                        self.ants[i].job = Job::ForageSurface;
                        self.ants[i].y = gy as f32;
                    }
                }

                Job::CarryFood => {
                    let target = food_room.or(Some((gw / 2, gy + 3)));

                    if let Some((nx, ny)) = plan_walk(
                        ax,
                        ay,
                        &grid_snapshot,
                        gw,
                        gh,
                        gy,
                        target,
                        false,
                        true,
                        &mut rng,
                    ) {
                        self.ants[i].x = nx as f32;
                        self.ants[i].y = ny as f32;

                        if grid_snapshot[ny][nx] == Cell::RoomFood || target == Some((nx, ny)) {
                            if self.ants[i].food_carried {
                                deliveries.push(FoodDelivery { ant_idx: i });
                            }
                        }
                    } else {
                        self.ants[i].job = Job::Wander;
                    }
                }

                Job::ForageSurface | Job::Queen | Job::Larva { .. } => {}
            }
        }

        // Apply deferred mutation requests.

        for DigReq { x, y } in dig_reqs {
            if y < self.grid_h && x < self.grid_w && self.grid[y][x] == Cell::Dirt {
                self.grid[y][x] = Cell::Tunnel;
                if rand::rng().random_bool(0.045) {
                    self.spark_burst(x as f32, y as f32, (120, 85, 45), 2);
                }
            }
        }

        for req in excav_reqs {
            match req.room {
                Cell::RoomFood if self.food_room_pos.is_none() => {
                    self.food_room_pos = Some((req.cx, req.cy));
                    self.excavate_room(req);
                }
                Cell::RoomQueen if self.queen_room_pos.is_none() => {
                    self.queen_room_pos = Some((req.cx, req.cy));
                    self.queen_pos = (req.cx, req.cy);
                    self.queen_alive = true;
                    self.queen_health = 1.0;
                    self.queen_lay_timer = 14.0;
                    self.excavate_room(req);
                }
                Cell::RoomNursery if self.nursery_pos.is_none() => {
                    self.nursery_pos = Some((req.cx, req.cy));
                    self.excavate_room(req);
                }
                _ => {}
            }
        }

        // Food pickups use snapshot indices, so sort descending and guard bounds.
        pickups.sort_by(|a, b| b.food_idx.cmp(&a.food_idx));
        let mut picked_food_indices: Vec<usize> = Vec::new();
        for pickup in pickups {
            if pickup.ant_idx < self.ants.len()
                && pickup.food_idx < self.food_items.len()
                && !picked_food_indices.contains(&pickup.food_idx)
            {
                picked_food_indices.push(pickup.food_idx);
                self.ants[pickup.ant_idx].job = Job::CarryFood;
                self.ants[pickup.ant_idx].food_carried = true;
                self.ants[pickup.ant_idx].y = self.ground_y as f32 + 1.0;
            }
        }

        picked_food_indices.sort_unstable();
        picked_food_indices.dedup();
        for idx in picked_food_indices.into_iter().rev() {
            if idx < self.food_items.len() {
                self.food_items.remove(idx);
            }
        }

        let mut delivered_count = 0u32;
        for delivery in deliveries {
            if delivery.ant_idx < self.ants.len() && self.ants[delivery.ant_idx].food_carried {
                self.ants[delivery.ant_idx].food_carried = false;
                self.ants[delivery.ant_idx].job = Job::Wander;
                self.ants[delivery.ant_idx].energy = 1.0;
                delivered_count += 1;
            }
        }

        if delivered_count > 0 {
            self.food_stored += delivered_count;
            if self.queen_alive {
                self.queen_health = (self.queen_health + QUEEN_GAIN * delivered_count as f32).min(1.0);
            }

            if let Some((fx, fy)) = self.food_room_pos {
                self.spark_burst(fx as f32, fy as f32, (255, 185, 60), 6);
            }
        }

        for kill in kills {
            if kill.ant_idx < self.ants.len() {
                self.ants[kill.ant_idx].alive = false;
            }
        }

        self.ants.extend(births);

        self.ants.retain(|a| a.alive);

        // Keep population healthy enough to watch.
        if self.ants.len() < 10 {
            let spawn = self
                .food_room_pos
                .or(self.deepest_open_cell())
                .unwrap_or((self.grid_w / 2, self.ground_y + 2));

            let mut rng = rand::rng();
            for _ in 0..(12 - self.ants.len()).min(4) {
                self.ants.push(Ant {
                    x: spawn.0 as f32 + rng.random_range(-1.0..1.0),
                    y: spawn.1 as f32,
                    step_timer: 0.0,
                    job: if rng.random_bool(0.35) { Job::Dig } else { Job::Wander },
                    alive: true,
                    food_carried: false,
                    energy: 0.8,
                });
            }
        }

        for spark in &mut self.sparks {
            spark.x += spark.vx * dt_f;
            spark.y += spark.vy * dt_f;
            spark.vy += 7.0 * dt_f;
            spark.life -= dt_f;
        }

        self.sparks.retain(|s| s.life > 0.0);

        if self.sparks.len() > 260 {
            let drop = self.sparks.len() - 260;
            self.sparks.drain(..drop);
        }
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
        let gy = self.ground_y;
        let total_depth = h.saturating_sub(gy).max(1) as f64;
        let seed = self.noise_seed;

        let mut out = String::with_capacity(w * h * 28);
        let mut last_fg: Option<Rgb> = None;
        let mut last_bg: Option<Rgb> = None;

        for row in 0..h {
            for col in 0..w {
                let cell = if col < self.grid_w && row < self.grid_h {
                    self.grid[row][col]
                } else {
                    Cell::Sky
                };

                let depth = if row > gy {
                    (row - gy) as f64 / total_depth
                } else {
                    0.0
                };

                let (ch, fg, bg): (char, Rgb, Rgb) = match cell {
                    Cell::Sky => {
                        let t = row as f64 / gy.max(1) as f64;
                        let c = lerp_rgb((8, 70, 185), (78, 150, 235), t);
                        (' ', c, c)
                    }
                    Cell::Grass => {
                        let wave = (col as f64 * 0.42 + t_abs * 1.1).sin() * 0.5 + 0.5;
                        let g = (58 + (wave * 50.0) as u8).min(155);
                        let c = (13, g, 10);
                        let ch = if (col.wrapping_mul(7) + 3) % 5 == 0 { '\'' } else { '▓' };
                        (ch, c, c)
                    }
                    Cell::Entrance => {
                        let c = (58, 44, 27);
                        ('░', c, c)
                    }
                    Cell::Dirt => {
                        let c = dirt_rgb(col, row, depth, seed);
                        let n = stable_noise(col, row, seed);
                        let ch = if n > 0.72 { '▒' } else if n > 0.35 { '█' } else { '▓' };
                        (ch, c, c)
                    }
                    Cell::Tunnel => {
                        let warm = ((1.0 - depth * 2.5).max(0.0) * 8.0) as u8;
                        let bg = (9 + warm, 7, 5);
                        (' ', bg, bg)
                    }
                    Cell::RoomFood => {
                        let bg = (22, 11, 5);
                        let n = (col.wrapping_mul(3).wrapping_add(row.wrapping_mul(7))) % 10;
                        if n == 0 && self.food_stored > 0 {
                            let dense = self.food_stored > 7
                                && (col.wrapping_mul(5).wrapping_add(row.wrapping_mul(3))) % 4 == 0;
                            if dense {
                                ('◆', (220, 145, 45), bg)
                            } else {
                                ('·', (180, 126, 48), bg)
                            }
                        } else {
                            (' ', bg, bg)
                        }
                    }
                    Cell::RoomQueen => {
                        let health = if self.queen_alive { self.queen_health as f64 } else { 0.12 };
                        let pulse = ((t_abs * 0.72).sin() * 0.5 + 0.5) * 18.0;
                        let base = 19.0 + health * 22.0;
                        let bg = ((base + pulse) as u8, 11, 4);
                        (' ', bg, bg)
                    }
                    Cell::RoomNursery => {
                        let bg = (5, 8, 24);
                        let is_egg = (col.wrapping_add(row.wrapping_mul(5))) % 13 == 0;
                        if is_egg {
                            ('○', (112, 166, 238), bg)
                        } else {
                            (' ', bg, bg)
                        }
                    }
                };

                if Some(bg) != last_bg {
                    out.push_str(&format!("\x1b[48;2;{};{};{}m", bg.0, bg.1, bg.2));
                    last_bg = Some(bg);
                }

                if Some(fg) != last_fg {
                    out.push_str(&format!("\x1b[38;2;{};{};{}m", fg.0, fg.1, fg.2));
                    last_fg = Some(fg);
                }

                out.push(ch);
            }

            out.push_str(RESET);
            last_fg = None;
            last_bg = None;

            if row < h - 1 {
                out.push('\n');
            }
        }

        self.render_overlays(&mut out, w, h, t_abs);

        out.push_str(RESET);
        out
    }
}

impl AntColonyMode {
    fn render_overlays(&self, out: &mut String, w: usize, h: usize, _t_abs: f64) {
        match self.surface_evt {
            SurfaceEvt::Stomp { cx, radius, age } => {
                let row = self.ground_y as i32;
                let r = (radius + age * 3.0).round() as i32;

                for dx in -r..=r {
                    let x = cx as i32 + dx;
                    if x >= 1 && (x as usize) < w {
                        out.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;2;255;220;90m·",
                            row + 1,
                            x + 1
                        ));
                    }
                }
            }
            SurfaceEvt::Bird { x, .. } => {
                let bx = x.round() as i32;
                let by = self.ground_y as i32;

                for &(dx, dy, ch) in &[(-2, -1, '~'), (-1, -1, '\\'), (0, 0, '▲'), (1, -1, '/'), (2, -1, '~')] {
                    let fx = bx + dx;
                    let fy = by + dy;

                    if fx >= 1 && (fx as usize) < w && fy >= 1 && (fy as usize) < h {
                        out.push_str(&format!(
                            "\x1b[{};{}H\x1b[38;2;35;25;18m{}",
                            fy + 1,
                            fx + 1,
                            ch
                        ));
                    }
                }
            }
            SurfaceEvt::None => {}
        }

        for food in &self.food_items {
            let fx = food.x.clamp(1, w.saturating_sub(1));
            let fy = food.y.clamp(1, h.saturating_sub(1));
            let br = ((food.age as f64 * 2.4).sin() * 0.5 + 0.5) * 0.38 + 0.62;
            let r = (255.0 * br) as u8;
            let g = (155.0 * br) as u8;

            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;{};{};4m◆",
                fy + 1,
                fx + 1,
                r,
                g
            ));
        }

        for ant in &self.ants {
            let ax = ant.x.round() as usize;
            let ay = ant.y.round() as usize;

            if ax == 0 || ay == 0 || ax >= w || ay >= h {
                continue;
            }

            let (ch, r, g, b) = match ant.job {
                Job::CarryFood => ('◈', 255, 198, 55),
                Job::ForageUp | Job::ForageSurface => ('↑', 212, 176, 82),
                Job::Dig => ('▼', 158, 118, 58),
                Job::Queen => ('♛', 255, 215, 0),
                Job::Larva { timer } => {
                    let f = ((timer / LARVA_HATCH) * std::f32::consts::TAU * 3.0).sin() * 0.5 + 0.5;
                    let v = (140.0 + f as f64 * 80.0) as u8;
                    ('✦', v, v, 240)
                }
                Job::Wander => ('◗', 192, 148, 62),
            };

            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;{};{};{}m{}",
                ay + 1,
                ax + 1,
                r,
                g,
                b,
                ch
            ));
        }

        for spark in &self.sparks {
            let sx = spark.x.round() as usize;
            let sy = spark.y.round() as usize;

            if sx == 0 || sy == 0 || sx >= w || sy >= h {
                continue;
            }

            let f = (spark.life / spark.max_life).clamp(0.0, 1.0);

            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;{};{};{}m✦",
                sy + 1,
                sx + 1,
                (spark.col.0 as f32 * f) as u8,
                (spark.col.1 as f32 * f) as u8,
                (spark.col.2 as f32 * f) as u8,
            ));
        }

        let queen_str = if self.succession_active {
            format!("succession {:>2}s", self.succession_timer.max(0.0) as i32)
        } else if self.queen_alive {
            let hearts = (self.queen_health * 5.0).round() as usize;
            format!("queen {:<5}", "♥".repeat(hearts.min(5)))
        } else {
            "no queen  ".to_string()
        };

        let workers = self
            .ants
            .iter()
            .filter(|a| !matches!(a.job, Job::Queen | Job::Larva { .. }))
            .count();

        let larvae = self
            .ants
            .iter()
            .filter(|a| matches!(a.job, Job::Larva { .. }))
            .count();

        let rooms = [
            if self.food_room_pos.is_some() { "food" } else { "·" },
            if self.queen_room_pos.is_some() { "queen" } else { "·" },
            if self.nursery_pos.is_some() { "nursery" } else { "·" },
        ]
        .join(" ");

        out.push_str(&format!(
            "\x1b[1;1H\x1b[48;2;10;6;3m\x1b[38;2;210;166;75m Ant Colony  {:<14}  w:{:>3} l:{:>2} food:{:>3}  rooms:[{}]\x1b[0m",
            queen_str,
            workers,
            larvae,
            self.food_stored,
            rooms
        ));

        let phase = if self.nursery_pos.is_some() {
            "brood cycle"
        } else if self.queen_room_pos.is_some() {
            "queen founded"
        } else if self.food_room_pos.is_some() {
            "foraging"
        } else {
            "digging"
        };

        out.push_str(&format!(
            "\x1b[2;1H\x1b[48;2;10;6;3m\x1b[38;2;150;120;70m phase:{:<14} ants:{:<3} age:{:>4}s\x1b[0m",
            phase,
            self.ants.len(),
            self.colony_age as i32
        ));
    }
}

fn nearest_food_from_snapshot(food: &[(usize, usize)], x: usize) -> Option<usize> {
    food.iter()
        .enumerate()
        .min_by_key(|(_, (fx, _))| (*fx as i32 - x as i32).abs())
        .map(|(i, _)| i)
}
