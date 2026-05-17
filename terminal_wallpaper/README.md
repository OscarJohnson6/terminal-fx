# Terminal Wallpaper — Rust Port

A Rust port of the Python `terminal_wallpaper` project.

## Building & Running

```bash
# Debug build (fast compile, slower runtime)
cargo run

# Release build (optimized — use for the actual animation)
cargo run --release
```

## Project Layout

```
src/
├── main.rs          ← Entry point + run loop (replaces main.py)
├── ansi.rs          ← ANSI escape constants  (replaces ansi.py)
├── color.rs         ← ColorProvider          (replaces color.py)
├── menu.rs          ← User prompts           (replaces menu.py)
├── mode_base.rs     ← `Mode` trait           (replaces ModeBase class)
└── modes/
    ├── mod.rs           ← Module exports
    └── shooting_stars.rs ← ShootingStarMode  (replaces shootingstars.py)
```

## Key Python → Rust Translations

| Python | Rust |
|--------|------|
| `class Foo:` | `struct Foo { }` + `impl Foo { }` |
| `class Foo(Base):` | `struct Foo` + `impl Trait for Foo` |
| `Optional[T]` | `Option<T>` |
| `list` | `Vec<T>` |
| `dict` | `HashMap<K, V>` or a static slice of tuples |
| `f"text {x}"` | `format!("text {}", x)` |
| `x % n` (always positive) | `x.rem_euclid(n)` |
| `random.uniform(a, b)` | `rng.gen_range(a..b)` |
| Exception handling | `Result<T, E>` + `?` operator |
| `try/finally` | `Drop` trait (runs on scope exit) |
| Duck typing | `dyn Trait` (trait objects) |
| `None` | `None` (part of `Option<T>`) |

## Adding More Modes

1. Create `src/modes/your_mode.rs`
2. Define a `pub struct YourMode { ... }`
3. `impl Mode for YourMode { fn update(...) fn render(...) }`
4. In `src/modes/mod.rs`, add `pub mod your_mode;` and `pub use your_mode::YourMode;`
5. In `src/main.rs`, add a match arm in `build_mode()`:
   ```rust
   "your_key" => Some(Box::new(YourMode::new(speed, color_provider))),
   ```

The `ShootingStarMode` in `modes/shooting_stars.rs` is the fully-commented
reference — read it alongside `shootingstars.py` to see the translation patterns.


adding modes in the Modes/ folder then add a reference to them in mod.rs then list them in the menu to be displayed and then in main rs to its actually selected.
