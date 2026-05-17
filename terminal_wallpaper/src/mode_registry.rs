// ===================================================================
//  src/mode_registry.rs
// -------------------------------------------------------------------
//  Self-describing mode registry.
//
//  THE CORE IDEA
//  Previously, adding a mode meant editing four things:
//    1. src/modes/mod.rs          — declare the submodule
//    2. here (imports)            — add a use statement
//    3. here (builders)           — add a mode_builder! call
//    4. here (MODES array)        — add a full ModeEntry literal
//
//  Now you edit two things:
//    1. src/modes/mod.rs          — declare the submodule (unavoidable;
//                                   Rust's module system requires it)
//    2. The BOTTOM of this file   — one line in register_all!()
//
//  HOW IT WORKS
//  Each mode type implements `ModeDescriptor` — a companion trait
//  to the runtime `Mode` trait. ModeDescriptor carries the static
//  metadata (id, name, desc, fps) that used to live in the MODES
//  array literal. The `register_all!` macro reads that trait to
//  build the ModeEntry for you, so you never type the id/name/desc
//  more than once.
//
//  WHY NOT A PROC MACRO OR INVENTORY CRATE?
//  A proc-macro #[derive(ModeEntry)] would feel cleaner but
//  requires a separate crate and build.rs complexity. The `inventory`
//  crate enables true zero-touch registration but adds a dependency
//  and uses linker tricks. This approach is self-contained, compile-
//  time safe, and can be understood without reading external docs.
//
//  ADDING A MODE (full workflow)
//  ─────────────────────────────
//  1. Create  src/modes/my_mode.rs
//  2. In that file, impl ModeDescriptor for MyModeType { ... }
//     (see the template comment below)
//  3. In src/modes/mod.rs, add:  pub mod my_mode;
//  4. In this file, add ONE LINE inside register_all!:
//         my_mode::MyModeType,
//
//  That's it. No import, no builder function, no ModeEntry literal.
// ===================================================================

use crate::color::ColorProvider;
use crate::mode_base::Mode;

// ── Public types ─────────────────────────────────────────────────────────────

pub type ModeBuilder = fn(f64, ColorProvider) -> Box<dyn Mode>;

/// Runtime metadata for a mode, plus its constructor function.
/// Stored in the `MODES` static slice and used by the menu and CLI.
pub struct ModeEntry {
    pub id:    &'static str,
    pub name:  &'static str,
    pub desc:  &'static str,
    pub fps:   u32,
    pub build: ModeBuilder,
}

// ── ModeDescriptor trait ──────────────────────────────────────────────────────
//
// Implement this trait on your mode struct — once — and the registry
// reads everything it needs from it.
//
// Template (copy into your new mode file):
//
//   impl ModeDescriptor for MyMode {
//       const ID:   &'static str = "mymode";
//       const NAME: &'static str = "My Mode";
//       const DESC: &'static str = "Short description for the menu";
//       const FPS:  u32          = 60;
//   }
//
// The ID is also used for CLI --mode lookups, so keep it lowercase
// and hyphen-friendly (e.g. "neon_subway", "castle").

pub trait ModeDescriptor {
    const ID:   &'static str;
    const NAME: &'static str;
    const DESC: &'static str;
    const FPS:  u32;
}

// ── register_all! macro ───────────────────────────────────────────────────────
//
// Expands a comma-separated list of fully-qualified mode types into:
//   - one `use` statement per type
//   - one `ModeEntry` per type (metadata read from ModeDescriptor)
//   - the `MODES` static slice
//   - find_mode() and menu_entries() helpers
//
// The builder function is generated inline so you never write one by hand.
// The explicit `use crate::modes::...` imports are replaced by a single
// `use crate::modes::*` at the call site, handled inside the macro arm.

macro_rules! register_all {
    ( $( $module:ident :: $type:ident ),+ $(,)? ) => {

        // Pull every registered mode type into scope.
        $( use crate::modes::$module::$type; )+

        // The static registry. Populated entirely from ModeDescriptor constants.
        pub static MODES: &[ModeEntry] = &[
            $( ModeEntry {
                id:    <$type as ModeDescriptor>::ID,
                name:  <$type as ModeDescriptor>::NAME,
                desc:  <$type as ModeDescriptor>::DESC,
                fps:   <$type as ModeDescriptor>::FPS,
                build: |speed, cp| Box::new(<$type>::new(speed, cp)),
            }, )+
        ];

        /// Look up a mode by id or alias. Case-insensitive.
        pub fn find_mode(id: &str) -> Option<&'static ModeEntry> {
            let key = id.to_ascii_lowercase();
            // Backward-compatible aliases — add new ones here as needed.
            let key = match key.as_str() {
                "cystal_vine"                   => "crystal_vine",
                "volcano_island"
                | "volcano_islands"             => "volcano",
                "subway"                        => "neon_subway",
                "tidal"                         => "tidal_beach",
                "factions" | "faction_war"      => "faction_wars",
                "train"                         => "train_journey",
                other => other,
            };
            MODES.iter().find(|e| e.id == key)
        }

        /// Build the flat (id, name, desc) list used by the menu.
        pub fn menu_entries() -> Vec<(&'static str, &'static str, &'static str)> {
            MODES.iter().map(|e| (e.id, e.name, e.desc)).collect()
        }
    };
}

// ── Mode implementations of ModeDescriptor ────────────────────────────────────
//
// These blocks live here, not in the mode files themselves, so that all
// metadata is visible in one place and easy to compare / sort.
//
// If you strongly prefer the metadata to live alongside the mode code,
// move each `impl ModeDescriptor for …` block into its own file — the
// registry doesn't care either way as long as the trait is in scope.

mod descriptors {
    use super::ModeDescriptor;
    use crate::modes::{
        aurora_city::AuroraCityMode,
        beach_shore::BeachShoreMode,
        biome_colony::BiomeColonyMode,
        bounce::BounceMode,
        castle_siege::CastleSiegeMode,
        crystal_vine::CrystalVineMode,
        faction_wars::FactionWarsMode,
        fish_tank::FishTankMode,
        landscape::LandscapeMode,
        matrix::MatrixMode,
        metaballs::MetaballsMode,
        neon_subway::NeonSubwayMode,
        plasma::PlasmaMode,
        rain::RainMode,
        rebirth_city::RebirthCityMode,
        shooting_stars::ShootingStarMode,
        sky_harbor::SkyHarborMode,
        space_ship::SpaceShipMode,
        tidal_beach::TidalBeachMode,
        train_journey::TrainJourneyMode,
        tree::TreeMode,
        void_garden::VoidGardenMode,
        volcano_islands::VolcanoIslandsMode,
        warp::WarpMode,
        langtons_ants::AntColonyMode,
        boid_flock::BoidFlockMode,
        reaction_diffusion::ReactionDiffusionMode,
        oven_meatballs::OvenMeatballsMode,
        street_parallax::StreetParallaxMode,
        factory_supply_run::FactorySupplyRunMode,
        spirit_grove::SpiritGroveMode,
        lava_lamp::LavaLampMode,
        potion_lab::PotionLabMode,
    };

    // ── One impl block per mode ───────────────────────────────────────────────
    // Sorted alphabetically by ID for easy scanning.

    impl ModeDescriptor for AuroraCityMode {
        const ID:   &'static str = "aurora";
        const NAME: &'static str = "Aurora City";
        const DESC: &'static str = "Futuristic skyline and aurora";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for BeachShoreMode {
        const ID:   &'static str = "beach_shore";
        const NAME: &'static str = "Beach Shore";
        const DESC: &'static str = "ASCII shoreline scene";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for BiomeColonyMode {
        const ID:   &'static str = "biome";
        const NAME: &'static str = "Biome Colony";
        const DESC: &'static str = "Alien terrarium ecosystem";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for BounceMode {
        const ID:   &'static str = "bounce";
        const NAME: &'static str = "Bounce";
        const DESC: &'static str = "Kinetic physics simulation";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for CastleSiegeMode {
        const ID:   &'static str = "castle";
        const NAME: &'static str = "Castle Siege";
        const DESC: &'static str = "Medieval battle with catapults";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for CrystalVineMode {
        const ID:   &'static str = "crystal_vine";
        const NAME: &'static str = "Crystal Vine";
        const DESC: &'static str = "Shimmering vine and crystals";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for FactionWarsMode {
        const ID:   &'static str = "faction_wars";
        const NAME: &'static str = "Faction Wars";
        const DESC: &'static str = "Endless global domination cycle";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for FishTankMode {
        const ID:   &'static str = "fishtank";
        const NAME: &'static str = "Fishtank";
        const DESC: &'static str = "Bio-organic aquatic sim";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for LandscapeMode {
        const ID:   &'static str = "landscape";
        const NAME: &'static str = "Landscape";
        const DESC: &'static str = "Atmospheric day/night scene";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for MatrixMode {
        const ID:   &'static str = "matrix";
        const NAME: &'static str = "Matrix";
        const DESC: &'static str = "Encrypted data stream";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for MetaballsMode {
        const ID:   &'static str = "metaballs";
        const NAME: &'static str = "Metaballs";
        const DESC: &'static str = "Lava-lamp organic blobs";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for NeonSubwayMode {
        const ID:   &'static str = "neon_subway";
        const NAME: &'static str = "Neon Subway";
        const DESC: &'static str = "Cyber tunnel with trains";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for PlasmaMode {
        const ID:   &'static str = "plasma";
        const NAME: &'static str = "Plasma";
        const DESC: &'static str = "Demo-scene sine plasma";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for RainMode {
        const ID:   &'static str = "rain";
        const NAME: &'static str = "Rain";
        const DESC: &'static str = "Storm with lightning";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for RebirthCityMode {
        const ID:   &'static str = "rebirth";
        const NAME: &'static str = "Rebirth City";
        const DESC: &'static str = "City destruction and regrowth";
        const FPS:  u32          = 45;
    }
    impl ModeDescriptor for ShootingStarMode {
        const ID:   &'static str = "stars";
        const NAME: &'static str = "Stars";
        const DESC: &'static str = "Hyperspace particle field";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for SkyHarborMode {
        const ID:   &'static str = "skyharbor";
        const NAME: &'static str = "Sky Harbor";
        const DESC: &'static str = "Floating islands with airships";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for SpaceShipMode {
        const ID:   &'static str = "ship";
        const NAME: &'static str = "Ship";
        const DESC: &'static str = "Predictive BFS navigation";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for TidalBeachMode {
        const ID:   &'static str = "tidal_beach";
        const NAME: &'static str = "Tidal Beach";
        const DESC: &'static str = "Smooth beach with washing tide";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for TrainJourneyMode {
        const ID:   &'static str = "train_journey";
        const NAME: &'static str = "Train Journey";
        const DESC: &'static str = "Scenic steam locomotive ride";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for TreeMode {
        const ID:   &'static str = "tree";
        const NAME: &'static str = "Tree";
        const DESC: &'static str = "Seed to sapling to full tree";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for VoidGardenMode {
        const ID:   &'static str = "voidgarden";
        const NAME: &'static str = "Void Garden";
        const DESC: &'static str = "Black-hole garden with blooms";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for VolcanoIslandsMode {
        const ID:   &'static str = "volcano";
        const NAME: &'static str = "Volcano Islands";
        const DESC: &'static str = "ASCII eruptions and lava";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for WarpMode {
        const ID:   &'static str = "warp";
        const NAME: &'static str = "Warp";
        const DESC: &'static str = "Hyperspace star tunnel";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for AntColonyMode {
        const ID:   &'static str = "langtons_ants";
        const NAME: &'static str = "Langton's Ants";
        const DESC: &'static str = "Emergent Turing machine";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for BoidFlockMode {
        const ID:   &'static str = "boid_flock";
        const NAME: &'static str = "Boid Flock";
        const DESC: &'static str = "Flocking birds with obstacles";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for ReactionDiffusionMode {
        const ID:   &'static str = "reaction_diffusion";
        const NAME: &'static str = "Reaction Diffusion";
        const DESC: &'static str = "Chemical reaction simulation";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for OvenMeatballsMode {
        const ID:   &'static str = "meatballs";
        const NAME: &'static str = "Oven Meatballs";
        const DESC: &'static str = "Oven cam with wheat-free foods";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for StreetParallaxMode {
        const ID:   &'static str = "street_parallax";
        const NAME: &'static str = "Street Parallax";
        const DESC: &'static str = "City street with parallax layers";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for FactorySupplyRunMode {
        const ID:   &'static str = "factory_supply_run";
        const NAME: &'static str = "Factory Supply Run";
        const DESC: &'static str = "Dynamic factory supply chain simulation";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for SpiritGroveMode {
        const ID:   &'static str = "spirit_grove";
        const NAME: &'static str = "Spirit Grove";
        const DESC: &'static str = "Mystical grove with floating spirits";
        const FPS:  u32          = 60;
    }
    impl ModeDescriptor for LavaLampMode {
        const ID:   &'static str = "lava_lamp";
        const NAME: &'static str = "Lava Lamp";
        const DESC: &'static str = "Classic lava lamp simulation";
        const FPS:  u32          = 50;
    }
    impl ModeDescriptor for PotionLabMode {
        const ID:   &'static str = "potion_lab";
        const NAME: &'static str = "Potion Lab";
        const DESC: &'static str = "Alchemy lab with bubbling potions";
        const FPS:  u32          = 50;
    }
}

// ── Registration ─────────────────────────────────────────────────────────────
//
// TO ADD A MODE: append one line here, e.g.  my_module::MyModeType,
// Everything else (imports, builder, MODES entry) is derived automatically.

register_all!(
    aurora_city::AuroraCityMode,
    beach_shore::BeachShoreMode,
    biome_colony::BiomeColonyMode,
    bounce::BounceMode,
    castle_siege::CastleSiegeMode,
    crystal_vine::CrystalVineMode,
    faction_wars::FactionWarsMode,
    fish_tank::FishTankMode,
    landscape::LandscapeMode,
    matrix::MatrixMode,
    metaballs::MetaballsMode,
    neon_subway::NeonSubwayMode,
    plasma::PlasmaMode,
    rain::RainMode,
    rebirth_city::RebirthCityMode,
    shooting_stars::ShootingStarMode,
    sky_harbor::SkyHarborMode,
    space_ship::SpaceShipMode,
    tidal_beach::TidalBeachMode,
    train_journey::TrainJourneyMode,
    tree::TreeMode,
    void_garden::VoidGardenMode,
    volcano_islands::VolcanoIslandsMode,
    warp::WarpMode,
    langtons_ants::AntColonyMode,
    boid_flock::BoidFlockMode,
    reaction_diffusion::ReactionDiffusionMode,
    oven_meatballs::OvenMeatballsMode,
    street_parallax::StreetParallaxMode,
    factory_supply_run::FactorySupplyRunMode,
    spirit_grove::SpiritGroveMode,
    lava_lamp::LavaLampMode,
    potion_lab::PotionLabMode,
);