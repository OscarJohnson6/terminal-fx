// ===================================================================
//  src/modes/faction_wars.rs
// -------------------------------------------------------------------
//  Endless faction-domination simulation.
//
//  CONCEPT
//   A grid of territories, each owned by one of up to 8 colour-coded
//   factions (or unclaimed). Factions expand outward from their
//   capitals, fight along their borders, and reshape the map over
//   thousands of simulated years. The simulation never settles —
//   the longer a faction dominates, the more aggressively the world
//   pushes back against it.
//
//  ANTI-STALEMATE DESIGN
//   Three independent mechanisms combine to keep the simulation
//   visually interesting forever, without ever scripting a "winner":
//
//   1. Negative feedback on dominance.
//      When any faction passes ~62% of the map, the random-event
//      lottery starts strongly biasing toward events that target
//      that faction specifically: plagues hit them first, civil
//      wars split them, meteors fall on their territory.
//
//   2. Faction personalities.
//      Each faction has an aggression score that's set at spawn.
//      Aggressive factions push borders; defensive ones consolidate.
//      Mixed personalities create natural ebb-and-flow even without
//      external events.
//
//   3. Periodic "great upheavals".
//      Roughly every 3-5 minutes of simulated time, a Cleansing
//      event resets the map. Old factions die, new ones spawn,
//      biomes shift slightly. This is the safety valve that
//      prevents very-long-run convergence.
//
//  RENDERING
//   Each tile is rendered as a 2×2 pixel block in a half-block
//   framebuffer. Tile colour = faction colour modulated by current
//   strength. Capital tiles glow with a brighter centre. Recently
//   contested tiles flicker. Battle particles spawn on takeovers.
// ===================================================================

use crate::ansi::{rgb, RESET};
use crate::color::ColorProvider;
use crate::mode_base::Mode;
use rand::RngExt;

type Rgb = (u8, u8, u8);

// ── Tunable constants ──────────────────────────────────────────────

const MAX_FACTIONS: usize = 8;
const SPREAD_INTERVAL: f32 = 0.04; // faster sim ticks; keeps the map moving
const STRENGTH_REGEN: f32 = 9.0;    // lower regen so borders do not become concrete
const CAPITAL_REGEN_BONUS: f32 = 20.0; // extra regen for capital tiles
const STRENGTH_MAX: f32 = 100.0;
const STRENGTH_TILE_INIT: f32 = 55.0;

const DOMINANCE_TARGET_THRESHOLD: f32 = 0.48; // events bias against above this
const APOCALYPSE_THRESHOLD: f32 = 0.68;        // forces a major event above this

const TILE_W: usize = 2; // pixels per tile horizontally
const TILE_H: usize = 2; // pixels per tile vertically (so 1 terminal row)

const HUD_RESERVED_ROWS: usize = 2; // top terminal rows kept for HUD

// Biomes
const BIOME_PLAINS: u8 = 0;
const BIOME_HILLS: u8 = 1;
const BIOME_MOUNT: u8 = 2;
const BIOME_WATER: u8 = 3;
const BIOME_DESERT: u8 = 4;

// Event types are stored as discriminants for compactness.
enum EventState {
    None,
    Plague { centre_x: f32, centre_y: f32, radius: f32, victim: u8, age: f32, max_age: f32 },
    Meteors { age: f32, strikes_remaining: u8, target_faction: u8, period: f32, last_strike: f32 },
    CivilWar { parent: u8, breakaway: u8, age: f32, max_age: f32 },
    NewPower { faction_id: u8, age: f32, max_age: f32 },
    Cleansing { age: f32, max_age: f32 },
}

// ── Faction & tile data ────────────────────────────────────────────

#[derive(Clone, Copy)]
struct Faction {
    id: u8,
    color: Rgb,
    name: &'static str,
    aggression: f32, // 0.5 = passive, 1.5 = ruthless
    alive: bool,
    capital: (i32, i32),
}

#[derive(Clone, Copy)]
struct Tile {
    owner: u8,        // 0 = neutral
    strength: f32,    // 0..STRENGTH_MAX
    contested: f32,   // 0..1, decays each frame
    is_capital: bool,
    biome: u8,
}

struct Particle {
    x: f32, y: f32,
    vx: f32, vy: f32,
    life: f32, max_life: f32,
    col: Rgb,
}

// Built-in factions to draw from when spawning. We always randomise
// the order and pick a subset, so two consecutive runs feel different.
const FACTION_PALETTE: &[(Rgb, &str)] = &[
    ((220, 60, 70),    "Crimson"),
    ((60, 130, 235),   "Azure"),
    ((90, 200, 80),    "Verdant"),
    ((255, 195, 70),   "Solar"),
    ((180, 130, 245),  "Violet"),
    ((110, 220, 220),  "Frost"),
    ((240, 100, 180),  "Rose"),
    ((205, 130, 60),   "Bronze"),
    ((140, 200, 240),  "Sky"),
    ((230, 230, 230),  "Pearl"),
    ((100, 70, 50),    "Umber"),
    ((180, 40, 120),   "Plum"),
];

const BIOME_COLORS: &[Rgb] = &[
    (54, 90, 50),    // plains
    (78, 96, 56),    // hills
    (105, 95, 80),   // mountains
    (38, 70, 130),   // water
    (180, 160, 100), // desert
];

// ── The mode ────────────────────────────────────────────────────────

pub struct FactionWarsMode {
    speed: f64,
    _color: ColorProvider,

    grid: Vec<Vec<Tile>>,
    grid_w: usize,
    grid_h: usize,

    factions: Vec<Faction>,
    next_faction_id: u8,
    particles: Vec<Particle>,

    spread_accum: f32,
    year: f32,                  // arbitrary in-fiction time
    event_cooldown: f32,        // seconds until next random event eligible
    apocalypse_cooldown: f32,   // gates the huge "Cleansing" event
    event: EventState,

    initialized: bool,
    last_resize: (u16, u16),
}

impl FactionWarsMode {
    pub fn new(speed: f64, color: ColorProvider) -> Self {
        Self {
            speed,
            _color: color,
            grid: Vec::new(),
            grid_w: 0,
            grid_h: 0,
            factions: Vec::new(),
            next_faction_id: 1,
            particles: Vec::new(),
            spread_accum: 0.0,
            year: 1000.0,
            event_cooldown: 8.0,
            apocalypse_cooldown: 65.0, // much faster safety valve
            event: EventState::None,
            initialized: false,
            last_resize: (0, 0),
        }
    }

    /// Build a fresh world: terrain, factions, capitals.
    fn init_world(&mut self, width: u16, height: u16) {
        let pix_w = width as usize;
        let pix_h = (height as usize).saturating_sub(HUD_RESERVED_ROWS) * 2;

        self.grid_w = pix_w / TILE_W;
        self.grid_h = pix_h / TILE_H;
        self.last_resize = (width, height);

        // Generate biome map with summed sines so adjacent tiles tend
        // to share biomes — gives natural-looking continents.
        let mut rng = rand::rng();
        let seed = rng.random_range(0.0..1000.0);

        self.grid = (0..self.grid_h)
            .map(|r| {
                (0..self.grid_w)
                    .map(|c| {
                        let biome = sample_biome(c as f64, r as f64, seed);
                        Tile {
                            owner: 0,
                            strength: 0.0,
                            contested: 0.0,
                            is_capital: false,
                            biome,
                        }
                    })
                    .collect()
            })
            .collect();

        // Spawn factions — pick a random subset of the palette.
        self.factions.clear();
        self.next_faction_id = 1;
        let count = rng.random_range(5..=7);
        let mut palette: Vec<usize> = (0..FACTION_PALETTE.len()).collect();
        // Manual shuffle so we don't take a SliceRandom dependency.
        for i in (1..palette.len()).rev() {
            let j = rng.random_range(0..=i);
            palette.swap(i, j);
        }

        for &p_idx in palette.iter().take(count) {
            self.spawn_faction(FACTION_PALETTE[p_idx].0, FACTION_PALETTE[p_idx].1);
        }

        self.initialized = true;
    }

    /// Place a brand new faction somewhere on a non-water tile.
    fn spawn_faction(&mut self, color: Rgb, name: &'static str) -> Option<u8> {
        let mut rng = rand::rng();
        // Up to 80 attempts to find an unclaimed land tile.
        for _ in 0..80 {
            let r = rng.random_range(0..self.grid_h);
            let c = rng.random_range(0..self.grid_w);
            let tile = self.grid[r][c];
            if tile.owner != 0 || tile.biome == BIOME_WATER {
                continue;
            }

            let id = self.next_faction_id;
            self.next_faction_id = self.next_faction_id.wrapping_add(1).max(1);

            let aggression = rng.random_range(0.70..1.55);
            self.factions.push(Faction {
                id,
                color,
                name,
                aggression,
                alive: true,
                capital: (c as i32, r as i32),
            });

            // Capital + starter zone. A larger seed makes the opening less slow.
            self.grid[r][c] = Tile {
                owner: id,
                strength: STRENGTH_MAX,
                contested: 0.0,
                is_capital: true,
                biome: tile.biome,
            };
            for _ in 0..9 {
                let dr = rng.random_range(-1i32..=1);
                let dc = rng.random_range(-1i32..=1);
                let nr = (r as i32 + dr).clamp(0, self.grid_h as i32 - 1) as usize;
                let nc = (c as i32 + dc).clamp(0, self.grid_w as i32 - 1) as usize;
                if self.grid[nr][nc].biome != BIOME_WATER {
                    self.grid[nr][nc] = Tile {
                        owner: id,
                        strength: STRENGTH_TILE_INIT,
                        contested: 0.0,
                        is_capital: false,
                        biome: self.grid[nr][nc].biome,
                    };
                }
            }
            return Some(id);
        }
        None
    }

    /// Look up a faction by id. Returns None for neutral (id=0) or dead.
    fn faction(&self, id: u8) -> Option<&Faction> {
        if id == 0 { return None; }
        self.factions.iter().find(|f| f.id == id && f.alive)
    }

    fn faction_mut(&mut self, id: u8) -> Option<&mut Faction> {
        if id == 0 { return None; }
        self.factions.iter_mut().find(|f| f.id == id && f.alive)
    }

    /// Count tiles owned by each faction.
    fn tile_counts(&self) -> Vec<(u8, usize)> {
        let total = self.grid_h * self.grid_w;
        let mut counts = vec![0usize; (self.next_faction_id as usize).max(1) + 1];
        for row in &self.grid {
            for t in row {
                if (t.owner as usize) < counts.len() {
                    counts[t.owner as usize] += 1;
                }
            }
        }
        let _ = total;
        self.factions
            .iter()
            .filter(|f| f.alive)
            .map(|f| (f.id, counts.get(f.id as usize).copied().unwrap_or(0)))
            .collect()
    }

    fn dominance(&self) -> Option<(u8, f32)> {
        let total = self.land_tile_count() as f32;
        if total <= 0.0 { return None; }
        self.tile_counts()
            .into_iter()
            .map(|(id, n)| (id, n as f32 / total))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    }

    fn land_tile_count(&self) -> usize {
        self.grid
            .iter()
            .flat_map(|row| row.iter())
            .filter(|t| t.biome != BIOME_WATER)
            .count()
            .max(1)
    }

    fn counts_array(&self) -> Vec<usize> {
        let mut counts = vec![0usize; (self.next_faction_id as usize).max(16) + 2];
        for row in &self.grid {
            for t in row {
                let idx = t.owner as usize;
                if idx < counts.len() {
                    counts[idx] += 1;
                }
            }
        }
        counts
    }

    fn share_from_counts(counts: &[usize], id: u8, total: f32) -> f32 {
        counts.get(id as usize).copied().unwrap_or(0) as f32 / total.max(1.0)
    }

    fn spawn_breakthrough(&mut self, r: usize, c: usize, owner: u8, power: f32) {
        let mut rng = rand::rng();
        let dirs = [
            (0i32, -1i32), (0, 1), (-1, 0), (1, 0),
            (-1, -1), (1, -1), (-1, 1), (1, 1),
        ];

        let attempts = if power > 42.0 { 4 } else if power > 25.0 { 3 } else { 2 };

        for _ in 0..attempts {
            let (dr, dc) = dirs[rng.random_range(0..dirs.len())];
            let nr = r as i32 + dr;
            let nc = c as i32 + dc;

            if nr < 0 || nr >= self.grid_h as i32 || nc < 0 || nc >= self.grid_w as i32 {
                continue;
            }

            let nr = nr as usize;
            let nc = nc as usize;
            let t = self.grid[nr][nc];

            if t.owner == owner || t.biome == BIOME_WATER || t.is_capital {
                continue;
            }

            // Breakthroughs only eat neutral/weak/exhausted tiles.
            if t.owner == 0 || t.strength < power * 1.15 || rng.random_bool(0.18) {
                self.grid[nr][nc].owner = owner;
                self.grid[nr][nc].strength = rng.random_range(18.0..55.0);
                self.grid[nr][nc].contested = 1.0;
                self.grid[nr][nc].is_capital = false;

                let px = nc as f32 * TILE_W as f32 + TILE_W as f32 * 0.5;
                let py = nr as f32 * TILE_H as f32 + TILE_H as f32 * 0.5
                      + (HUD_RESERVED_ROWS as f32 * 2.0);
                for _ in 0..2 {
                    let ang: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                    let spd: f32 = rng.random_range(5.0..12.0);
                    self.particles.push(Particle {
                        x: px, y: py,
                        vx: ang.cos() * spd,
                        vy: ang.sin() * spd * 0.55,
                        life: rng.random_range(0.18..0.48),
                        max_life: 0.48,
                        col: (255, 150, 80),
                    });
                }
            }
        }
    }

    // ── Simulation tick ───────────────────────────────────────────
    fn spread_tick(&mut self) {
        let mut rng = rand::rng();
        let h = self.grid_h;
        let w = self.grid_w;
        let counts = self.counts_array();
        let total_land = self.land_tile_count() as f32;

        // Iterate in a randomly-rotated order each tick so no faction
        // gets a positional advantage from being scanned first.
        let row_offset = rng.random_range(0..h.max(1));
        let col_offset = rng.random_range(0..w.max(1));

        for ri in 0..h {
            for ci in 0..w {
                let r = (ri + row_offset) % h;
                let c = (ci + col_offset) % w;

                let tile = self.grid[r][c];
                if tile.owner == 0 || tile.strength < 30.0 { continue; }

                let aggression = self.faction(tile.owner)
                    .map(|f| f.aggression)
                    .unwrap_or(1.0);

                let owner_share = Self::share_from_counts(&counts, tile.owner, total_land);

                // The old version let huge blobs harden. This adds two forces:
                // underdogs attack harder, overextended empires attack worse.
                let underdog_boost = if owner_share < 0.16 {
                    1.0 + (0.16 - owner_share) * 3.0
                } else {
                    1.0
                };

                let empire_fatigue = if owner_share > 0.42 {
                    1.0 - (owner_share - 0.42).min(0.35) * 1.25
                } else {
                    1.0
                };

                // Probability of attempting to spread this tick.
                let attempt = (0.25 * aggression * underdog_boost * empire_fatigue).clamp(0.03, 0.92);
                if !rng.random_bool(attempt as f64) {
                    continue;
                }

                let dirs = [(0i32, -1i32), (0, 1), (-1, 0), (1, 0)];
                let (dr, dc) = dirs[rng.random_range(0..4)];
                let nr = r as i32 + dr;
                let nc = c as i32 + dc;
                if nr < 0 || nr >= h as i32 || nc < 0 || nc >= w as i32 { continue; }
                let nr = nr as usize;
                let nc = nc as usize;

                let n_owner = self.grid[nr][nc].owner;
                let n_strength = self.grid[nr][nc].strength;
                let n_biome = self.grid[nr][nc].biome;

                if n_owner == tile.owner { continue; }
                if n_biome == BIOME_WATER { continue; }

                // Terrain affects the attacker's effective power.
                let terrain_factor = match n_biome {
                    BIOME_MOUNT => 0.55,
                    BIOME_HILLS => 0.80,
                    BIOME_DESERT => 0.90,
                    _ => 1.0,
                };

                let defender_share = Self::share_from_counts(&counts, n_owner, total_land);
                let defender_fatigue = if defender_share > 0.42 {
                    1.0 - (defender_share - 0.42).min(0.35) * 0.85
                } else {
                    1.0
                };

                let mut attack = tile.strength * 0.48 * aggression * terrain_factor;
                attack *= underdog_boost * empire_fatigue * rng.random_range(0.84..1.24);

                // Defenders get a flat resistance bonus + their strength.
                // Capitals still matter, but normal borders no longer become permanent walls.
                let mut defense = n_strength * 0.50 + 12.0;
                if self.grid[nr][nc].is_capital {
                    defense += 34.0;
                }
                defense *= defender_fatigue.max(0.58) * rng.random_range(0.82..1.18);

                if n_owner == 0 {
                    defense *= 0.62;
                }

                if attack > defense {
                    // Tile flips
                    self.grid[nr][nc].owner = tile.owner;
                    self.grid[nr][nc].strength =
                        ((attack - defense).max(15.0)).min(70.0);
                    self.grid[nr][nc].contested = 1.0;
                    self.grid[nr][nc].is_capital = false; // capitals never relocate this way

                    self.grid[r][c].strength *= 0.65;
                    self.grid[r][c].contested = 1.0;

                    // Battle spark — a couple of golden particles
                    let px = nc as f32 * TILE_W as f32 + TILE_W as f32 * 0.5;
                    let py = nr as f32 * TILE_H as f32 + TILE_H as f32 * 0.5
                          + (HUD_RESERVED_ROWS as f32 * 2.0);
                    for _ in 0..9 {
                        let ang: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                        let spd: f32 = rng.random_range(4.0..9.0);
                        self.particles.push(Particle {
                            x: px, y: py,
                            vx: ang.cos() * spd,
                            vy: ang.sin() * spd * 0.55,
                            life: rng.random_range(0.18..0.45),
                            max_life: 0.45,
                            col: (255, 220, 110),
                        });
                    }
                } else {
                    // Defender holds. Both sides bleed strength.
                    self.grid[r][c].strength *= 0.84;
                    self.grid[nr][nc].strength = (n_strength - rng.random_range(5.0..11.0)).max(4.0);
                    self.grid[r][c].contested = 1.0;
                    self.grid[nr][nc].contested = 1.0;
                }
            }
        }

        // Slow regeneration everywhere, faster at capitals.
        let dt = SPREAD_INTERVAL;
        for row in &mut self.grid {
            for t in row {
                if t.owner == 0 { continue; }
                let bonus = if t.is_capital { CAPITAL_REGEN_BONUS } else { 0.0 };
                t.strength = (t.strength + (STRENGTH_REGEN + bonus) * dt).min(STRENGTH_MAX);

                // Long-held non-capital territory slowly softens so old borders
                // do not become visually permanent. Capitals remain strong.
                if !t.is_capital && t.contested < 0.05 {
                    t.strength = (t.strength - dt * 0.9).max(24.0);
                }

                t.contested = (t.contested - dt * 1.7).max(0.0);
            }
        }

        // Detect dead factions: capital has been overrun.
        for f in self.factions.iter_mut().filter(|f| f.alive) {
            let (cx, cy) = f.capital;
            if cx >= 0 && cy >= 0 && (cx as usize) < self.grid_w && (cy as usize) < self.grid_h {
                let cap = self.grid[cy as usize][cx as usize];
                if cap.owner != f.id {
                    f.alive = false;
                }
            }
        }
    }

    // ── Events ────────────────────────────────────────────────────
    /// Pick a new event biased by the current dominance situation.
    /// If someone is dominating, events that hurt them are far more
    /// likely; this is the central anti-stalemate mechanism.
    fn roll_random_event(&mut self) {
        let dom = self.dominance();
        let dominant_id = dom.map(|(id, frac)| (id, frac));
        let mut rng = rand::rng();

        // Above APOCALYPSE_THRESHOLD, force a Cleansing — we never
        // let one faction snowball forever.
        if let Some((_, frac)) = dominant_id {
            if frac >= APOCALYPSE_THRESHOLD && self.apocalypse_cooldown <= 0.0 {
                self.start_cleansing();
                self.apocalypse_cooldown = 65.0;
                return;
            }
        }

        // Weighted lottery. Weights shift when there's a clear leader.
        let dom_pressure = dominant_id
            .map(|(_, frac)| (frac - DOMINANCE_TARGET_THRESHOLD).max(0.0) * 6.0)
            .unwrap_or(0.0);

        let weights = [
            ("plague",     2.2 + dom_pressure * 1.2),
            ("meteors",    1.8 + dom_pressure * 0.8),
            ("civilwar",   1.8 + dom_pressure * 1.4),
            ("newpower",   1.7),
            ("plague_sm",  1.3),
        ];
        let total: f32 = weights.iter().map(|w| w.1).sum();
        let mut roll = rng.random_range(0.0..total);
        let mut chosen = "plague";
        for (name, w) in &weights {
            if roll < *w { chosen = name; break; }
            roll -= *w;
        }

        match chosen {
            "plague" => self.start_plague(true,  dominant_id.map(|d| d.0)),
            "plague_sm" => self.start_plague(false, None),
            "meteors" => self.start_meteors(dominant_id.map(|d| d.0)),
            "civilwar" => self.start_civilwar(dominant_id.map(|d| d.0)),
            "newpower" => self.start_newpower(),
            _ => {}
        }
    }

    fn start_plague(&mut self, target_dominant: bool, dominant: Option<u8>) {
        let mut rng = rand::rng();
        let victim = if target_dominant {
            dominant.unwrap_or_else(|| self.random_alive_faction(&mut rng).unwrap_or(0))
        } else {
            self.random_alive_faction(&mut rng).unwrap_or(0)
        };
        if victim == 0 { return; }

        // Centre on a random tile owned by the victim.
        let mut centre = (self.grid_w as f32 / 2.0, self.grid_h as f32 / 2.0);
        for _ in 0..40 {
            let r = rng.random_range(0..self.grid_h);
            let c = rng.random_range(0..self.grid_w);
            if self.grid[r][c].owner == victim {
                centre = (c as f32, r as f32);
                break;
            }
        }

        self.event = EventState::Plague {
            centre_x: centre.0, centre_y: centre.1,
            radius: 0.0,
            victim,
            age: 0.0,
            max_age: 10.0,
        };
    }

    fn start_meteors(&mut self, target: Option<u8>) {
        let target_faction = target.unwrap_or(0);
        let mut rng = rand::rng();
        self.event = EventState::Meteors {
            age: 0.0,
            strikes_remaining: rng.random_range(8..15),
            target_faction,
            period: rng.random_range(0.35..0.75),
            last_strike: 0.0,
        };
    }

    fn start_civilwar(&mut self, victim: Option<u8>) {
        let mut rng = rand::rng();
        let parent_id = victim
            .or_else(|| self.random_alive_faction(&mut rng))
            .unwrap_or(0);
        if parent_id == 0 { return; }

        // Spawn the breakaway faction (uses an unused palette colour
        // shifted to a neighbouring hue).
        let parent_color = self.faction(parent_id).map(|f| f.color).unwrap_or((180, 180, 180));
        let break_color = (
            parent_color.0.saturating_add(40).saturating_sub(15),
            parent_color.1.saturating_add(15),
            parent_color.2.saturating_add(40),
        );
        let break_id = match self.spawn_faction(break_color, "Rebels") {
            Some(id) => id,
            None => return,
        };

        // Convert ~30% of the parent's tiles to the breakaway.
        let mut converted = 0;
        let mut limit = (self.tile_counts()
            .iter()
            .find(|(id, _)| *id == parent_id)
            .map(|(_, n)| *n)
            .unwrap_or(0) as f32 * 0.42) as usize;
        for r in 0..self.grid_h {
            for c in 0..self.grid_w {
                if self.grid[r][c].owner == parent_id && limit > 0 && !self.grid[r][c].is_capital && rng.random_bool(0.50) {
                    self.grid[r][c].owner = break_id;
                    self.grid[r][c].strength *= 0.7;
                    self.grid[r][c].contested = 1.0;
                    self.grid[r][c].is_capital = false;
                    converted += 1;
                    limit -= 1;
                }
            }
        }
        let _ = converted;

        self.event = EventState::CivilWar {
            parent: parent_id,
            breakaway: break_id,
            age: 0.0,
            max_age: 7.0,
        };
    }

    fn start_newpower(&mut self) {
        let mut rng = rand::rng();
        // Pick a colour not already in use.
        let used: Vec<Rgb> = self.factions.iter().filter(|f| f.alive).map(|f| f.color).collect();
        let candidates: Vec<&(Rgb, &str)> = FACTION_PALETTE.iter()
            .filter(|p| !used.contains(&p.0))
            .collect();
        if candidates.is_empty() { return; }
        let pick = candidates[rng.random_range(0..candidates.len())];

        let new_id = match self.spawn_faction(pick.0, pick.1) {
            Some(id) => id,
            None => return,
        };
        self.event = EventState::NewPower { faction_id: new_id, age: 0.0, max_age: 6.0 };
    }

    fn start_cleansing(&mut self) {
        self.event = EventState::Cleansing { age: 0.0, max_age: 7.5 };
    }

    fn random_alive_faction(&self, rng: &mut impl RngExt) -> Option<u8> {
        let alive: Vec<u8> = self.factions.iter().filter(|f| f.alive).map(|f| f.id).collect();
        if alive.is_empty() { None } else { Some(alive[rng.random_range(0..alive.len())]) }
    }

    fn tick_event(&mut self, dt: f32) {
        let mut finished = false;
        // Move event into a local — we'll mutate it and put it back
        // (or replace with None) at the end. This lets us call &mut
        // self methods without aliasing the event field.
        let mut ev = std::mem::replace(&mut self.event, EventState::None);
        match &mut ev {
            EventState::None => {}
            EventState::Plague { centre_x, centre_y, radius, victim, age, max_age } => {
                *age += dt;
                *radius += dt * 2.25;
                let mut rng = rand::rng();
                // Each tick, take some tiles inside the radius from the victim.
                for r in 0..self.grid_h {
                    for c in 0..self.grid_w {
                        if self.grid[r][c].owner != *victim { continue; }
                        let dx = c as f32 - *centre_x;
                        let dy = (r as f32 - *centre_y) * 2.0;
                        let d = (dx*dx + dy*dy).sqrt();
                        if d <= *radius && rng.random_bool(0.16) {
                            self.grid[r][c].owner = 0;
                            self.grid[r][c].strength = 0.0;
                            self.grid[r][c].contested = 1.0;
                            self.grid[r][c].is_capital = false;
                        }
                    }
                }
                if age >= max_age { finished = true; }
            }
            EventState::Meteors { age, strikes_remaining, target_faction, period, last_strike } => {
                *age += dt;
                if *age - *last_strike >= *period && *strikes_remaining > 0 {
                    *last_strike = *age;
                    *strikes_remaining -= 1;
                    let mut rng = rand::rng();
                    // Find a tile owned by the target if possible, else random.
                    let mut target = (
                        rng.random_range(0..self.grid_w),
                        rng.random_range(0..self.grid_h),
                    );
                    if *target_faction != 0 {
                        for _ in 0..30 {
                            let r = rng.random_range(0..self.grid_h);
                            let c = rng.random_range(0..self.grid_w);
                            if self.grid[r][c].owner == *target_faction {
                                target = (c, r);
                                break;
                            }
                        }
                    }
                    // Crater: larger wipe to actually break stalemates
                    let (tc, tr) = target;
                    for dr in -3i32..=3 {
                        for dc in -3i32..=3 {
                            let nr = tr as i32 + dr;
                            let nc = tc as i32 + dc;
                            if nr < 0 || nr >= self.grid_h as i32 || nc < 0 || nc >= self.grid_w as i32 { continue; }
                            if dr*dr + dc*dc > 8 { continue; }
                            self.grid[nr as usize][nc as usize].owner = 0;
                            self.grid[nr as usize][nc as usize].strength = 0.0;
                            self.grid[nr as usize][nc as usize].is_capital = false;
                        }
                    }
                    // Particle burst at impact site
                    let px = tc as f32 * TILE_W as f32 + TILE_W as f32 * 0.5;
                    let py = tr as f32 * TILE_H as f32 + TILE_H as f32 * 0.5
                          + (HUD_RESERVED_ROWS as f32 * 2.0);
                    for _ in 0..24 {
                        let ang: f32 = rng.random_range(0.0..std::f32::consts::TAU);
                        let spd: f32 = rng.random_range(4.0..14.0);
                        self.particles.push(Particle {
                            x: px, y: py,
                            vx: ang.cos() * spd,
                            vy: ang.sin() * spd * 0.55,
                            life: rng.random_range(0.4..0.9),
                            max_life: 0.9,
                            col: (255, rng.random_range(80u8..200u8), 0),
                        });
                    }
                }
                if *strikes_remaining == 0 && *age > 1.5 { finished = true; }
            }
            EventState::CivilWar { age, max_age, .. }
            | EventState::NewPower { age, max_age, .. } => {
                *age += dt;
                if age >= max_age { finished = true; }
            }
            EventState::Cleansing { age, max_age } => {
                *age += dt;
                // Halfway through, wipe the map and respawn factions.
                if *age >= *max_age * 0.5 && *age - dt < *max_age * 0.5 {
                    for row in &mut self.grid {
                        for t in row {
                            t.owner = 0;
                            t.strength = 0.0;
                            t.contested = 1.0;
                            t.is_capital = false;
                        }
                    }
                    self.factions.clear();
                    self.next_faction_id = 1;
                    let mut rng = rand::rng();
                    let count = rng.random_range(5..=7);
                    let mut palette: Vec<usize> = (0..FACTION_PALETTE.len()).collect();
                    for i in (1..palette.len()).rev() {
                        let j = rng.random_range(0..=i);
                        palette.swap(i, j);
                    }
                    for &p in palette.iter().take(count) {
                        self.spawn_faction(FACTION_PALETTE[p].0, FACTION_PALETTE[p].1);
                    }
                }
                if age >= max_age { finished = true; }
            }
        }
        if finished {
            self.event = EventState::None;
        } else {
            self.event = ev;
        }
    }
}

// ── Biome generator ────────────────────────────────────────────────

fn sample_biome(x: f64, y: f64, seed: f64) -> u8 {
    let n = 0.50 * (x * 0.07 + seed         ).sin()
          + 0.30 * (y * 0.09 + seed * 1.3   ).sin()
          + 0.20 * ((x + y) * 0.05 + seed*0.4).sin()
          + 0.10 * (x * 0.20 + y * 0.15     ).sin();
    if n < -0.55 { BIOME_WATER }
    else if n < -0.20 { BIOME_PLAINS }
    else if n <  0.15 { BIOME_HILLS }
    else if n <  0.45 { BIOME_DESERT }
    else { BIOME_MOUNT }
}

// ── Mode trait impl ─────────────────────────────────────────────────

impl Mode for FactionWarsMode {
    fn update(&mut self, dt: f64, width: u16, height: u16, _t_abs: f64) {
        let dt = dt * self.speed;

        // Re-init on first frame or terminal resize.
        if !self.initialized || self.last_resize != (width, height) {
            self.init_world(width, height);
        }

        let dt_f32 = dt as f32;
        self.year += dt_f32 * 12.0;
        self.spread_accum += dt_f32;
        self.event_cooldown -= dt_f32;
        self.apocalypse_cooldown -= dt_f32;

        // Run as many spread ticks as needed to keep up with real time
        // (capped so a stalled frame doesn't try to catch up indefinitely).
        let mut ticks_done = 0;
        while self.spread_accum >= SPREAD_INTERVAL && ticks_done < 4 {
            self.spread_accum -= SPREAD_INTERVAL;
            self.spread_tick();
            ticks_done += 1;
        }

        // Tick the active event (if any).
        if !matches!(self.event, EventState::None) {
            self.tick_event(dt_f32);
        } else {
            // Fast anti-stalemate pressure. If a faction is already large,
            // do not wait minutes to shake the world.
            if let Some((leader, frac)) = self.dominance() {
                if frac >= APOCALYPSE_THRESHOLD && self.apocalypse_cooldown <= 0.0 {
                    self.start_cleansing();
                    self.apocalypse_cooldown = 65.0;
                } else if frac >= 0.58 && self.event_cooldown <= 4.0 {
                    if rand::rng().random_bool(0.55) {
                        self.start_civilwar(Some(leader));
                    } else {
                        self.start_plague(true, Some(leader));
                    }
                    self.event_cooldown = 10.0;
                }
            }

            if matches!(self.event, EventState::None) && self.event_cooldown <= 0.0 {
                self.roll_random_event();
                // Cooldown until the *next* eligible event roll.
                let mut rng = rand::rng();
                self.event_cooldown = rng.random_range(8.0..16.0) / self.speed.max(0.4) as f32;
            }
        }

        if self.factions.iter().filter(|f| f.alive).count() < 4 && matches!(self.event, EventState::None) {
            self.start_newpower();
        }

        // Particles
        for p in &mut self.particles {
            p.x += p.vx * dt_f32;
            p.y += p.vy * dt_f32;
            p.vy += 14.0 * dt_f32;
            p.life -= dt_f32;
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    fn render(&self, width: u16, height: u16, t_abs: f64) -> String {
        let w = width as usize;
        let h = height as usize;
        let ph = h * 2;

        let mut pix: Vec<Vec<Rgb>> = vec![vec![(8, 10, 14); w]; ph];

        // ── Map ───────────────────────────────────────────────────
        let map_offset_y = HUD_RESERVED_ROWS * 2;
        for r in 0..self.grid_h {
            for c in 0..self.grid_w {
                let tile = self.grid[r][c];

                let base = if tile.owner == 0 {
                    BIOME_COLORS[tile.biome as usize % BIOME_COLORS.len()]
                } else {
                    self.faction(tile.owner)
                        .map(|f| f.color)
                        .unwrap_or((90, 90, 90))
                };

                // Modulate by strength so worn-down tiles look paler.
                let s = (tile.strength / STRENGTH_MAX).clamp(0.25, 1.0) as f64;
                let mut col = (
                    (base.0 as f64 * (0.55 + 0.45 * s)) as u8,
                    (base.1 as f64 * (0.55 + 0.45 * s)) as u8,
                    (base.2 as f64 * (0.55 + 0.45 * s)) as u8,
                );

                // Recently contested tiles flash a hot edge.
                if tile.contested > 0.05 {
                    let pulse = ((t_abs * 14.0).sin() * 0.5 + 0.5) * tile.contested as f64;
                    col = (
                        ((col.0 as f64 * (1.0 - pulse * 0.6)) + 255.0 * pulse * 0.6) as u8,
                        ((col.1 as f64 * (1.0 - pulse * 0.6)) + 220.0 * pulse * 0.6) as u8,
                        ((col.2 as f64 * (1.0 - pulse * 0.6)) +  80.0 * pulse * 0.6) as u8,
                    );
                }

                // Capital tiles get a brighter highlight on their centre.
                let fill_col = col;
                let cap_col = if tile.is_capital {
                    (
                        col.0.saturating_add(60),
                        col.1.saturating_add(60),
                        col.2.saturating_add(60),
                    )
                } else {
                    fill_col
                };

                // Paint the tile's pixel block.
                for dy in 0..TILE_H {
                    for dx in 0..TILE_W {
                        let py = map_offset_y + r * TILE_H + dy;
                        let px = c * TILE_W + dx;
                        if py < ph && px < w {
                            let centre = dx == TILE_W / 2 && dy == TILE_H / 2;
                            pix[py][px] = if centre && tile.is_capital { cap_col } else { fill_col };
                        }
                    }
                }
            }
        }

        // ── Plague aura overlay ───────────────────────────────────
        if let EventState::Plague { centre_x, centre_y, radius, .. } = &self.event {
            for r in 0..self.grid_h {
                for c in 0..self.grid_w {
                    let dx = c as f32 - *centre_x;
                    let dy = (r as f32 - *centre_y) * 2.0;
                    let d = (dx*dx + dy*dy).sqrt();
                    let edge_dist = (d - *radius).abs();
                    if edge_dist > 1.5 { continue; }
                    let alpha = (1.0 - edge_dist / 1.5).clamp(0.0, 1.0);
                    for dy in 0..TILE_H {
                        for dx in 0..TILE_W {
                            let py = map_offset_y + r * TILE_H + dy;
                            let px = c * TILE_W + dx;
                            if py < ph && px < w {
                                let bg = pix[py][px];
                                pix[py][px] = (
                                    (bg.0 as f32 * (1.0 - alpha * 0.7) + 180.0 * alpha * 0.7) as u8,
                                    (bg.1 as f32 * (1.0 - alpha * 0.7) +  40.0 * alpha * 0.7) as u8,
                                    (bg.2 as f32 * (1.0 - alpha * 0.7) + 120.0 * alpha * 0.7) as u8,
                                );
                            }
                        }
                    }
                }
            }
        }

        // ── Cleansing white-out fade ─────────────────────────────
        if let EventState::Cleansing { age, max_age } = &self.event {
            let prog = (age / max_age).clamp(0.0, 1.0);
            // Bell curve: peak whiteness at midpoint
            let intensity = (prog * std::f32::consts::PI).sin();
            for row in pix.iter_mut() {
                for c in row.iter_mut() {
                    *c = (
                        (c.0 as f32 * (1.0 - intensity * 0.85) + 255.0 * intensity * 0.85) as u8,
                        (c.1 as f32 * (1.0 - intensity * 0.85) + 250.0 * intensity * 0.85) as u8,
                        (c.2 as f32 * (1.0 - intensity * 0.85) + 230.0 * intensity * 0.85) as u8,
                    );
                }
            }
        }

        // ── Particles ────────────────────────────────────────────
        for p in &self.particles {
            let px = p.x as i32;
            let py = p.y as i32;
            if px >= 0 && (px as usize) < w && py >= 0 && (py as usize) < ph {
                let fade = (p.life / p.max_life).clamp(0.0, 1.0);
                pix[py as usize][px as usize] = (
                    (p.col.0 as f32 * fade) as u8,
                    (p.col.1 as f32 * fade) as u8,
                    (p.col.2 as f32 * fade) as u8,
                );
            }
        }

        // ── Compose half-block frame ─────────────────────────────
        let mut out = String::with_capacity(w * h * 42);
        for row in 0..h {
            for col in 0..w {
                let (ur, ug, ub) = pix[row * 2][col];
                let (lr, lg, lb) = if row * 2 + 1 < ph { pix[row * 2 + 1][col] } else { (0, 0, 0) };
                out.push_str(&format!(
                    "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m\u{2580}",
                    ur, ug, ub, lr, lg, lb,
                ));
            }
            out.push_str(RESET);
            if row < h - 1 { out.push('\n'); }
        }

        // ── HUD overlay ──────────────────────────────────────────
        // The HUD is positioned with absolute cursor moves, drawn AFTER
        // the framebuffer so it sits on top. Reserved rows above were
        // skipped in map painting so this doesn't ever cover content.
        out.push_str(&self.render_hud(width));

        out
    }
}

impl FactionWarsMode {
    fn render_hud(&self, width: u16) -> String {
        let counts = self.tile_counts();
        let total: usize = counts.iter().map(|(_, n)| *n).sum::<usize>().max(1);

        // Sort by tile count descending so leader is leftmost.
        let mut sorted = counts;
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        // Year line
        let year = self.year as i64;
        let event_text = match &self.event {
            EventState::None => String::from(""),
            EventState::Plague { victim, .. } => {
                let name = self.faction(*victim).map(|f| f.name).unwrap_or("?");
                format!(" · Plague in {}", name)
            }
            EventState::Meteors { target_faction, .. } => {
                let name = self.faction(*target_faction).map(|f| f.name).unwrap_or("the lands");
                format!(" · Meteor strikes batter {}", name)
            }
            EventState::CivilWar { parent, .. } => {
                let name = self.faction(*parent).map(|f| f.name).unwrap_or("?");
                format!(" · Civil war within {}", name)
            }
            EventState::NewPower { faction_id, .. } => {
                let name = self.faction(*faction_id).map(|f| f.name).unwrap_or("a banner");
                format!(" · {} rises", name)
            }
            EventState::Cleansing { .. } => String::from(" · The Cleansing"),
        };

        let mut hud = format!(
            "\x1b[1;1H\x1b[48;2;14;16;24m\x1b[38;2;225;235;245m Year {}{:width$}",
            year, event_text, width = (width as usize).saturating_sub(format!(" Year {}", year).len())
        );

        // Bar line: each faction shown as colored block + percentage.
        hud.push_str("\x1b[2;1H\x1b[48;2;14;16;24m");
        let mut bar = String::from(" ");
        for (id, n) in sorted.iter().take(MAX_FACTIONS) {
            let f = match self.faction(*id) {
                Some(f) => f,
                None => continue,
            };
            let pct = (*n as f32 / total as f32 * 100.0) as i32;
            bar.push_str(&format!(
                "{}{}{} {:>3}%  ",
                rgb(f.color.0, f.color.1, f.color.2),
                "▰▰",
                "\x1b[38;2;225;235;245m",
                pct,
            ));
        }
        // Pad / trim to terminal width
        let visible_len_estimate = bar.chars().filter(|c| !matches!(c, '\x1b')).count();
        let _ = visible_len_estimate;
        hud.push_str(&bar);
        // Fill any remainder with the HUD background
        hud.push_str(&format!(
            "{:<width$}",
            "",
            width = (width as usize).saturating_sub(120).min(width as usize)
        ));
        hud.push_str(RESET);
        hud
    }
}
