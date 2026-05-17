// ===================================================================
//  src/main.rs
// -------------------------------------------------------------------
//  TerminalFX entry point.
//
//  Responsibilities of this module:
//   - Parse CLI arguments (--mode, --theme, --speed, --fps, --list)
//   - Drive the interactive menu loop
//   - Run the per-frame render loop with input handling
//   - Manage terminal state (raw mode, alternate screen, cursor)
//   - Composite the help overlay on top of the active mode
//
//  Modes themselves live under `src/modes/` and are registered in
//  `src/mode_registry.rs`. Adding a new mode does not require any
//  changes here.
//
//  Frame loop architecture:
//   We build the entire frame (animation + overlay + sync escapes)
//   into a single reused String buffer, then issue exactly one
//   write_all + flush per frame. Combined with DEC Synchronized
//   Output (escape `?2026h/l`), this guarantees the terminal sees
//   each frame as one atomic update — no tearing, no flicker between
//   the animation and the help HUD. See run_mode_live() for details.
// ===================================================================

mod ansi;
mod color;
mod menu;
mod mode_base;
mod mode_registry;
mod modes;

use std::io::{self, Write};
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

use color::{ColorMode, ColorProvider};
use mode_base::Mode;
use mode_registry::{find_mode, menu_entries, ModeEntry, MODES};

// ── CLI configuration ──────────────────────────────────────────────

/// Parsed command-line arguments. Everything is optional; missing
/// values fall back to the interactive menu (or hard defaults if
/// `--mode` is supplied without companions).
#[derive(Debug, Clone)]
struct LaunchConfig {
    mode: Option<String>,
    theme: Option<ColorMode>,
    speed: Option<f64>,
    fps: Option<u32>,
    list_modes: bool,
}

/// Live state shared between the menu, the frame loop, and runtime
/// hotkeys. `Copy` so we can rebuild a mode after a state change
/// without worrying about ownership.
#[derive(Clone, Copy)]
struct RuntimeState {
    speed: f64,
    theme: ColorMode,
    fps: u32,
}

fn parse_args() -> LaunchConfig {
    let mut config = LaunchConfig {
        mode: None,
        theme: None,
        speed: None,
        fps: None,
        list_modes: false,
    };

    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mode" | "-m" => config.mode = args.next(),
            "--theme" | "--vibe" | "-t" => {
                config.theme = args.next().and_then(|v| parse_theme(&v));
            }
            "--speed" | "-s" => {
                config.speed = args.next().and_then(|v| parse_speed(&v));
            }
            "--fps" => {
                config.fps = args.next().and_then(|v| v.parse::<u32>().ok());
            }
            "--list" | "--list-modes" => config.list_modes = true,
            "--help" | "-h" => {
                print_cli_help();
                std::process::exit(0);
            }
            _ => {} // unknown args silently ignored — keep parsing
        }
    }

    config
}

/// Parse a theme name into a `ColorMode`. Returns `None` for
/// unrecognised input so the caller can fall back to a default.
fn parse_theme(value: &str) -> Option<ColorMode> {
    match value.to_ascii_lowercase().as_str() {
        "default" | "rainbow" => Some(ColorMode::Rainbow),
        "ocean" => Some(ColorMode::Ocean),
        "sunset" => Some(ColorMode::Sunset),
        "matrix" => Some(ColorMode::Matrix),
        _ => None,
    }
}

/// Parse a speed name or numeric multiplier. Negative values and zero
/// are rejected so we never end up with a frozen scene.
fn parse_speed(value: &str) -> Option<f64> {
    match value.to_ascii_lowercase().as_str() {
        "slow" => Some(0.5),
        "normal" | "default" => Some(1.0),
        "fast" => Some(2.0),
        "ludicrous" => Some(4.0),
        _ => value.parse::<f64>().ok().filter(|v| *v > 0.0),
    }
}

fn theme_name(theme: ColorMode) -> &'static str {
    match theme {
        ColorMode::Rainbow => "Default",
        ColorMode::Ocean => "Ocean",
        ColorMode::Sunset => "Sunset",
        ColorMode::Matrix => "Matrix",
    }
}

/// Cycle to the next theme — used by the live `T` hotkey.
fn next_theme(theme: ColorMode) -> ColorMode {
    match theme {
        ColorMode::Rainbow => ColorMode::Ocean,
        ColorMode::Ocean => ColorMode::Sunset,
        ColorMode::Sunset => ColorMode::Matrix,
        ColorMode::Matrix => ColorMode::Rainbow,
    }
}

fn print_cli_help() {
    println!("TerminalFX");
    println!();
    println!("Usage:");
    println!("  terminal_wallpaper");
    println!("  terminal_wallpaper --mode skyharbor --theme ocean --speed 1.0");
    println!("  terminal_wallpaper --mode volcano --fps 60");
    println!();
    println!("Options:");
    println!("  -m, --mode <id>       Launch a mode directly");
    println!("  -t, --theme <name>    default | ocean | sunset | matrix");
    println!("  -s, --speed <value>   slow | normal | fast | ludicrous | number");
    println!("      --fps <number>    Override the mode's recommended FPS");
    println!("      --list            List available modes");
    println!("  -h, --help            Show this help");
}

fn print_modes() {
    println!("Available modes:");
    for entry in MODES {
        println!("  {:<14} {:<22} {}", entry.id, entry.name, entry.desc);
    }
}

// ── Terminal screen lifecycle ──────────────────────────────────────
//
// The animation runs inside the alternate screen buffer with raw mode
// on (so we get keypresses immediately, no Enter required). The menu
// runs in normal cooked mode on the primary buffer. These two helpers
// transition cleanly between the two states.

fn restore_menu_screen(stdout: &mut io::Stdout) -> io::Result<()> {
    let _ = disable_raw_mode();
    execute!(
        stdout,
        LeaveAlternateScreen,
        Show,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;
    stdout.flush()?;
    Ok(())
}

fn enter_animation_screen(stdout: &mut io::Stdout) -> io::Result<()> {
    enable_raw_mode()?;
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;
    stdout.flush()?;
    Ok(())
}

// ── Entry point ────────────────────────────────────────────────────

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ansi::enable_ansi_on_windows();
    let args = parse_args();

    if args.list_modes {
        print_modes();
        return Ok(());
    }

    let mut stdout = io::stdout();

    // Direct-launch path: bypass the menu when --mode is given.
    if let Some(mode_id) = args.mode.as_deref() {
        let Some(entry) = find_mode(mode_id) else {
            eprintln!("Unknown mode: {mode_id}");
            eprintln!("Use --list to see available modes.");
            return Ok(());
        };

        let state = RuntimeState {
            theme: args.theme.unwrap_or(ColorMode::Rainbow),
            speed: args.speed.unwrap_or(1.0),
            fps: args.fps.unwrap_or(entry.fps).clamp(1, 240),
        };

        enter_animation_screen(&mut stdout)?;
        let _quit = run_mode_live(entry, state)?;
        restore_menu_screen(&mut stdout)?;
        return Ok(());
    }

    // Interactive path: menu → settings → animation → repeat.
    loop {
        restore_menu_screen(&mut stdout)?;

        let entries = menu_entries();
        let mode_name = match menu::ask_mode(&entries) {
            Some(m) => m,
            None => break,
        };

        let Some(entry) = find_mode(&mode_name) else {
            continue; // shouldn't happen, but degrade gracefully
        };

        let state = RuntimeState {
            theme: menu::ask_color_vibe(),
            speed: menu::ask_speed(),
            fps: args.fps.unwrap_or(entry.fps).clamp(1, 240),
        };

        enter_animation_screen(&mut stdout)?;
        let quit_entirely = run_mode_live(entry, state)?;
        if quit_entirely {
            break;
        }
    }

    restore_menu_screen(&mut stdout)?;
    Ok(())
}

/// Construct a fresh boxed mode for the given entry and state.
/// Called both at first launch and whenever a hotkey rebuilds the
/// mode (so it picks up changed speed / theme).
fn build_runtime_mode(entry: &'static ModeEntry, state: RuntimeState) -> Box<dyn Mode> {
    let color_provider = ColorProvider::new(state.theme, state.speed);
    (entry.build)(state.speed, color_provider)
}

// ── The frame loop ─────────────────────────────────────────────────
//
// FLICKER-FREE DESIGN
//   Every frame is composed into a single reused String:
//
//       SYNC_BEGIN  HOME  <animation>  RESET  [overlay]  SYNC_END
//
//   That string is sent to stdout with one write_all and one flush.
//   The terminal sees a single byte stream and, if it understands the
//   2026 escape, holds its repaint until SYNC_END arrives. The result
//   is one atomic frame update with no visible interleaving between
//   the animation and the help HUD.
//
//   Why not BufWriter?  A BufWriter with capacity smaller than a
//   true-colour frame (which can easily exceed 200 KB at 80×24) will
//   auto-flush mid-frame whenever its buffer fills. That intermediate
//   flush is exactly what causes "the overlay flickers over the
//   animation" symptoms — the terminal repaints between halves of the
//   frame. Holding stdout's lock and writing one big buffer avoids
//   that entirely.
//
// HOTKEYS
//   Q / Ctrl+C   quit the application
//   Esc / M      return to the menu
//   H / ?        toggle the pinned help overlay
//
//   When help is pinned, additional live controls unlock:
//     +/-        adjust speed (geometric ±15%)
//     T          cycle theme
//   Gating these behind the pinned help prevents accidental presses
//   from changing settings when the user is just glancing at the
//   startup hint.
fn run_mode_live(entry: &'static ModeEntry, mut state: RuntimeState) -> io::Result<bool> {
    // Lock stdout for the lifetime of this function so other threads
    // cannot interleave writes between our frame and our flush.
    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Reusable frame buffer. Sized to comfortably hold a true-colour
    // frame for very large terminals (~150 cols × 60 rows would be
    // around 1 MB worst case; 2 MB gives headroom).
    let mut frame_buf = String::with_capacity(2 * 1024 * 1024);

    let mut mode = build_runtime_mode(entry, state);
    let mut last_frame = Instant::now();
    let start_time = Instant::now();

    // Help overlay state. The startup hint shows for ~1.3s on entry,
    // then disappears unless the user pins it with H/?.
    let mut help_pinned = false;
    let mut startup_help_until = start_time + Duration::from_millis(1300);

    // Cache the rendered overlay string. The animation frame underneath
    // is rebuilt every frame (cheap at terminal resolution), but the
    // overlay only changes when settings change — so we rebuild it
    // lazily and reuse the string across frames.
    let mut cached_overlay = String::new();
    let mut overlay_dirty = true;

    loop {
        // ── Input handling ───────────────────────────────────────
        // Non-blocking poll: wait at most 1 ms before falling through
        // to render. This keeps frame pacing tight even when the user
        // isn't pressing anything.
        if event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Quit shortcuts.
                    if (key.modifiers == KeyModifiers::CONTROL
                        && key.code == KeyCode::Char('c'))
                        || key.code == KeyCode::Char('q')
                    {
                        return Ok(true);
                    }

                    // Return-to-menu shortcuts.
                    if key.code == KeyCode::Esc || key.code == KeyCode::Char('m') {
                        return Ok(false);
                    }

                    // Toggle pinned help.
                    if key.code == KeyCode::Char('h') || key.code == KeyCode::Char('?') {
                        help_pinned = !help_pinned;
                        overlay_dirty = true;
                    }

                    // Live tuning (only available while help is pinned).
                    if help_pinned {
                        match key.code {
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                state.speed = (state.speed * 1.15).clamp(0.10, 8.0);
                                mode = build_runtime_mode(entry, state);
                                last_frame = Instant::now();
                                overlay_dirty = true;
                            }
                            KeyCode::Char('-') | KeyCode::Char('_') => {
                                state.speed = (state.speed / 1.15).clamp(0.10, 8.0);
                                mode = build_runtime_mode(entry, state);
                                last_frame = Instant::now();
                                overlay_dirty = true;
                            }
                            KeyCode::Char('t') | KeyCode::Char('T') => {
                                state.theme = next_theme(state.theme);
                                mode = build_runtime_mode(entry, state);
                                last_frame = Instant::now();
                                overlay_dirty = true;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // ── Simulation tick + render ─────────────────────────────
        let frame_start = Instant::now();
        let dt = frame_start.duration_since(last_frame).as_secs_f64();
        last_frame = frame_start;
        let t_abs = frame_start.duration_since(start_time).as_secs_f64();

        let (width, height) = crossterm::terminal::size()?;
        mode.update(dt, width, height, t_abs);
        let frame = mode.render(width, height, t_abs);

        let show_startup_hint = Instant::now() < startup_help_until;
        let show_overlay = help_pinned || show_startup_hint;

        if overlay_dirty || show_startup_hint {
            cached_overlay = help_overlay(
                entry.name,
                entry.id,
                state.speed,
                theme_name(state.theme),
                state.fps,
                help_pinned,
            );
            overlay_dirty = false;
        }

        // ── Compose one atomic frame ─────────────────────────────
        // Order is important: SYNC_BEGIN must come before any cursor
        // move so the terminal knows to start buffering. SYNC_END is
        // the very last byte so the repaint commits everything we've
        // built up in this frame.
        frame_buf.clear();
        frame_buf.push_str(ansi::SYNC_BEGIN);
        frame_buf.push_str(ansi::HOME);
        frame_buf.push_str(&frame);
        frame_buf.push_str(ansi::RESET);
        if show_overlay {
            frame_buf.push_str(&cached_overlay);
        }
        frame_buf.push_str(ansi::SYNC_END);

        handle.write_all(frame_buf.as_bytes())?;
        handle.flush()?;

        // ── Frame pacing ─────────────────────────────────────────
        let elapsed = frame_start.elapsed();
        let target = ansi::frame_duration_for_fps(state.fps);
        if elapsed < target {
            std::thread::sleep(target - elapsed);
        }

        // Mark startup hint expired so subsequent frames don't keep
        // re-rendering it. We pull `startup_help_until` into the past
        // once so the `Instant::now() < startup_help_until` check
        // becomes a constant-time comparison from then on.
        if !help_pinned && startup_help_until < Instant::now() {
            startup_help_until = Instant::now() - Duration::from_secs(1);
        }
    }
}

/// Render the help / status panel that hovers in the top-left corner.
/// Position-anchored escapes (`\x1b[r;cH`) place the box at a fixed
/// location regardless of the animation underneath. The panel uses an
/// opaque background colour so it remains readable over any frame.
fn help_overlay(
    mode_name: &str,
    mode_id: &str,
    speed: f64,
    theme: &str,
    fps: u32,
    pinned: bool,
) -> String {
    let lock_text = if pinned {
        "+/- speed  T theme"
    } else {
        "press H for controls"
    };
    let status = if pinned { "OPEN" } else { "HINT" };

    format!(
        concat!(
            "\x1b[2;3H\x1b[48;2;8;10;18m\x1b[38;2;225;235;245m┌ TerminalFX ─────────────────────────┐",
            "\x1b[3;3H│ Status: \x1b[38;2;120;190;255m{:<27}\x1b[38;2;225;235;245m│",
            "\x1b[4;3H│ Mode:   \x1b[38;2;120;190;255m{:<27}\x1b[38;2;225;235;245m│",
            "\x1b[5;3H│ ID:     \x1b[38;2;165;175;190m{:<27}\x1b[38;2;225;235;245m│",
            "\x1b[6;3H│ Theme:  \x1b[38;2;255;190;120m{:<11}\x1b[38;2;225;235;245m Speed: {:>4.2}x │",
            "\x1b[7;3H│ FPS:    \x1b[38;2;165;175;190m{:<4}\x1b[38;2;225;235;245m  {:<22} │",
            "\x1b[8;3H│ H/? toggle help   M menu   Q quit  │",
            "\x1b[9;3H└─────────────────────────────────────┘\x1b[0m"
        ),
        status, mode_name, mode_id, theme, speed, fps, lock_text
    )
}