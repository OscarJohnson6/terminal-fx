// ===== src/menu.rs =====
//
// TerminalFX menu
//
// Design goals:
// - Arrow-key browsing stays the main path.
// - Menus with many modes use a scrolling window.
// - 1-9 selects visible rows, not absolute rows.
// - PgUp/PgDn/Home/End jump quickly.
// - No search box, no carousel, no extra friction.
// - No full-screen clear on every redraw.
// - Uses a small line-diff renderer so only changed rows are rewritten.
//   This avoids both ghost text and menu flicker.

use std::io::{self, BufWriter, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute, queue,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

use crate::ansi::RESET;
use crate::color::ColorMode;

// ── Style ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuStyle {
    Clean,
    Neon,
    Minimal,
}

impl MenuStyle {
    fn next(self) -> Self {
        match self {
            Self::Clean => Self::Neon,
            Self::Neon => Self::Minimal,
            Self::Minimal => Self::Clean,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Clean => "CLEAN",
            Self::Neon => "NEON",
            Self::Minimal => "MINIMAL",
        }
    }
}

// ── Palette ──────────────────────────────────────────────────────────────────

const FG: &str = "\x1b[38;2;225;230;240m";
const DIM: &str = "\x1b[38;2;110;120;135m";
const SOFT: &str = "\x1b[38;2;165;175;190m";
const ACCENT: &str = "\x1b[38;2;120;190;255m";
const SEL_BG: &str = "\x1b[48;2;40;60;100m";
const SEL_FG: &str = "\x1b[38;2;245;250;255m";

const NEON_C: &str = "\x1b[38;2;0;255;240m";
const NEON_M: &str = "\x1b[38;2;255;30;200m";
const NEON_D: &str = "\x1b[38;2;70;70;100m";
const NEON_SOFT: &str = "\x1b[38;2;120;180;210m";
const NEON_SB: &str = "\x1b[48;2;0;230;220m\x1b[38;2;0;0;0m";

const VISIBLE_ROWS: usize = 12;

// ── Windowing ────────────────────────────────────────────────────────────────

fn visible_window(len: usize, sel: usize) -> (usize, usize) {
    if len <= VISIBLE_ROWS {
        return (0, len);
    }

    let half = VISIBLE_ROWS / 2;
    let mut start = sel.saturating_sub(half);

    if start + VISIBLE_ROWS > len {
        start = len - VISIBLE_ROWS;
    }

    (start, start + VISIBLE_ROWS)
}

fn shortcut_for_visible_index(visible_i: usize) -> String {
    if visible_i < 9 {
        (visible_i + 1).to_string()
    } else {
        " ".to_string()
    }
}

// ── Renderers ────────────────────────────────────────────────────────────────

fn render_clean(title: &str, items: &[(&str, &str)], sel: usize, style_name: &str) -> String {
    let (start, end) = visible_window(items.len(), sel);
    let mut s = String::with_capacity(4096);

    s.push_str(&format!("\r\n  {}TerminalFX{}\x1b[K\r\n", ACCENT, RESET));
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", FG, title, RESET));
    s.push_str(&format!(
        "  {}showing {}-{} of {}{}\x1b[K\r\n",
        DIM,
        start + 1,
        end,
        items.len(),
        RESET
    ));
    s.push_str(&format!("  {}{}\x1b[K\r\n", DIM, "─".repeat(72)));

    // Stable marker row.
    let above = if start > 0 { "↑ more above" } else { "" };
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", DIM, above, RESET));

    // Stable fixed-size list area.
    for row in 0..VISIBLE_ROWS {
        let i = start + row;

        if i >= end {
            s.push_str("\x1b[K\r\n");
            continue;
        }

        let (name, desc) = items[i];
        let shortcut = shortcut_for_visible_index(row);

        if i == sel {
            s.push_str(&format!(
                "  {}{}▶ {:>2}. [{}] {:<20}{} {}{}{}\x1b[K\r\n",
                SEL_BG,
                SEL_FG,
                i + 1,
                shortcut,
                name,
                RESET,
                SOFT,
                desc,
                RESET
            ));
        } else {
            s.push_str(&format!(
                "    {}{:>2}.{} [{}] {}{:<20}{} {}{}{}\x1b[K\r\n",
                DIM,
                i + 1,
                RESET,
                shortcut,
                FG,
                name,
                RESET,
                DIM,
                desc,
                RESET
            ));
        }
    }

    // Stable marker row.
    let below = if end < items.len() { "↓ more below" } else { "" };
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", DIM, below, RESET));

    s.push_str(&format!("  {}{}\x1b[K\r\n", DIM, "─".repeat(72)));
    s.push_str(&format!(
        "  {}↑↓{} select  {}PgUp/PgDn{} jump  {}Home/End{} edge  {}1-9{} visible\x1b[K\r\n",
        ACCENT, RESET, ACCENT, RESET, ACCENT, RESET, ACCENT, RESET
    ));
    s.push_str(&format!(
        "  {}Enter{} run  {}Tab{} style [{}]  {}Esc/Q{} quit{}\x1b[K\r\n",
        ACCENT, RESET, ACCENT, RESET, style_name, ACCENT, RESET, RESET
    ));

    s
}

fn render_neon(title: &str, items: &[(&str, &str)], sel: usize, style_name: &str) -> String {
    let (start, end) = visible_window(items.len(), sel);
    let mut s = String::with_capacity(4096);

    s.push_str(&format!("\r\n  {}//{:━<68}{}\x1b[K\r\n", NEON_M, "", RESET));
    s.push_str(&format!("  {}▶ {}{}\x1b[K\r\n", NEON_C, title, RESET));
    s.push_str(&format!(
        "  {}// MODES {}-{} / {}{}\x1b[K\r\n",
        NEON_D,
        start + 1,
        end,
        items.len(),
        RESET
    ));
    s.push_str(&format!("  {}//{:━<68}{}\x1b[K\r\n", NEON_M, "", RESET));

    let above = if start > 0 { "▲ SIGNAL ABOVE" } else { "" };
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", NEON_D, above, RESET));

    for row in 0..VISIBLE_ROWS {
        let i = start + row;

        if i >= end {
            s.push_str("\x1b[K\r\n");
            continue;
        }

        let (name, desc) = items[i];
        let shortcut = shortcut_for_visible_index(row);

        if i == sel {
            s.push_str(&format!(
                "  {}[{:>2}]{} [{}] {} {:<20} {} {}{}{}\x1b[K\r\n",
                NEON_M,
                i + 1,
                RESET,
                shortcut,
                NEON_SB,
                name,
                RESET,
                NEON_C,
                desc,
                RESET
            ));
        } else {
            s.push_str(&format!(
                "    {}{:>2}.{} [{}] {}{:<20}{} {}{}{}\x1b[K\r\n",
                NEON_D,
                i + 1,
                RESET,
                shortcut,
                NEON_SOFT,
                name,
                RESET,
                NEON_D,
                desc,
                RESET
            ));
        }
    }

    let below = if end < items.len() { "▼ SIGNAL BELOW" } else { "" };
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", NEON_D, below, RESET));

    s.push_str(&format!("  {}{:─<72}{}\x1b[K\r\n", NEON_D, "", RESET));
    s.push_str(&format!(
        "  {}↑↓ NAV // PG JUMP // HOME/END EDGE // 1-9 VISIBLE{}\x1b[K\r\n",
        NEON_D, RESET
    ));
    s.push_str(&format!(
        "  {}ENTER RUN // TAB STYLE [{}] // ESC/Q QUIT{}\x1b[K\r\n",
        NEON_D, style_name, RESET
    ));

    s
}

fn render_minimal(title: &str, items: &[(&str, &str)], sel: usize, style_name: &str) -> String {
    let (start, end) = visible_window(items.len(), sel);
    let mut s = String::with_capacity(3072);

    s.push_str(&format!("\r\n  {}{}{}\x1b[K\r\n", FG, title, RESET));
    s.push_str(&format!(
        "  {}{}-{} / {}{}\x1b[K\r\n",
        DIM,
        start + 1,
        end,
        items.len(),
        RESET
    ));
    s.push_str("\x1b[K\r\n");

    let above = if start > 0 { "↑ more above" } else { "" };
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", DIM, above, RESET));

    for row in 0..VISIBLE_ROWS {
        let i = start + row;

        if i >= end {
            s.push_str("\x1b[K\r\n");
            continue;
        }

        let (name, desc) = items[i];
        let mark = if i == sel { ">" } else { " " };
        let shortcut = shortcut_for_visible_index(row);

        s.push_str(&format!(
            "  {} {:>2}. [{}] {}{:<20}{}  {}{}{}\x1b[K\r\n",
            mark,
            i + 1,
            shortcut,
            if i == sel { FG } else { DIM },
            name,
            RESET,
            DIM,
            desc,
            RESET
        ));
    }

    let below = if end < items.len() { "↓ more below" } else { "" };
    s.push_str(&format!("  {}{}{}\x1b[K\r\n", DIM, below, RESET));

    s.push_str("\x1b[K\r\n");
    s.push_str(&format!(
        "  {}↑↓ select · Pg jump · Home/End edge · 1-9 visible{}\x1b[K\r\n",
        DIM, RESET
    ));
    s.push_str(&format!(
        "  {}Enter run · Tab style [{}] · Esc/Q quit{}\x1b[K\r\n",
        DIM, style_name, RESET
    ));

    s
}

fn render_menu(title: &str, items: &[(&str, &str)], sel: usize, style: MenuStyle) -> String {
    match style {
        MenuStyle::Clean => render_clean(title, items, sel, style.name()),
        MenuStyle::Neon => render_neon(title, items, sel, style.name()),
        MenuStyle::Minimal => render_minimal(title, items, sel, style.name()),
    }
}

// ── Diff drawing ─────────────────────────────────────────────────────────────

fn split_lines(frame: &str) -> Vec<String> {
    // Use '\n' because the frame already contains CRLF-ish terminal control.
    // trim_end_matches avoids adding a phantom empty line from a final newline.
    frame
        .trim_end_matches('\n')
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

fn draw_frame_diff(
    stdout: &mut BufWriter<io::Stdout>,
    previous: &mut Vec<String>,
    title: &str,
    items: &[(&str, &str)],
    sel: usize,
    style: MenuStyle,
) -> io::Result<()> {
    let frame = render_menu(title, items, sel, style);
    let current = split_lines(&frame);

    let max_lines = previous.len().max(current.len());

    for row in 0..max_lines {
        let old = previous.get(row);
        let new = current.get(row);

        // 1. Keep the optimization for standard menus
        if old == new {
            continue;
        }

        // 2. MOVE and WRITE only. DO NOT CLEAR. 
        // Overwriting is flicker-free; Clearing is not.
        queue!(stdout, MoveTo(0, row as u16))?;
        if let Some(line) = new {
            write!(stdout, "{}", line)?;
        }
    }

    *previous = current;
    
    // Flush EVERYTHING to the screen in one single, clean frame update
    stdout.flush()?;

    Ok(())
}

// ── Core selector ─────────────────────────────────────────────────────────────

fn arrow_select(title: &str, items: &[(&str, &str)]) -> io::Result<Option<usize>> {
    if items.is_empty() {
        return Ok(None);
    }

    let mut stdout = BufWriter::new(io::stdout());
    enable_raw_mode()?;

    // One clear when entering the alternate screen is fine.
    // The flicker came from clearing the whole screen on every selection move.
    execute!(
        stdout,
        EnterAlternateScreen,
        Hide,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;
    stdout.flush()?;

    let mut sel = 0usize;
    let mut style = MenuStyle::Clean;
    let mut previous_lines: Vec<String> = Vec::new();

    draw_frame_diff(&mut stdout, &mut previous_lines, title, items, sel, style)?;

    let result = loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let old_sel = sel;
            let old_style = style;

            let (start, end) = visible_window(items.len(), sel);

            match key.code {
                KeyCode::Up => sel = sel.checked_sub(1).unwrap_or(items.len() - 1),
                KeyCode::Down => sel = (sel + 1) % items.len(),
                KeyCode::PageUp => sel = sel.saturating_sub(VISIBLE_ROWS),
                KeyCode::PageDown => sel = (sel + VISIBLE_ROWS).min(items.len() - 1),
                KeyCode::Home => sel = 0,
                KeyCode::End => sel = items.len() - 1,
                KeyCode::Tab => style = style.next(),
                KeyCode::Enter => break Some(sel),
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => break None,
                KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                    let visible_idx = (c as u8 - b'1') as usize;
                    let idx = start + visible_idx;
                    if idx < end && idx < items.len() {
                        sel = idx;
                    }
                }
                _ => continue,
            }

            if sel != old_sel || style != old_style {
                draw_frame_diff(&mut stdout, &mut previous_lines, title, items, sel, style)?;
            }
        }
    };

    disable_raw_mode()?;
    execute!(stdout, Show, LeaveAlternateScreen)?;
    stdout.flush()?;

    Ok(result)
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn ask_mode(entries: &[(&str, &str, &str)]) -> Option<String> {
    let items: Vec<(&str, &str)> = entries.iter().map(|(_, n, d)| (*n, *d)).collect();
    let idx = arrow_select("Select wallpaper mode", &items).ok()??;
    Some(entries[idx].0.to_string())
}

pub fn ask_color_vibe() -> ColorMode {
    let items: &[(&str, &str)] = &[
        ("Default", "Balanced / recommended"),
        ("Ocean", "Cool blue-green"),
        ("Sunset", "Warm orange-red"),
        ("Matrix", "Phosphor green"),
    ];

    match arrow_select("Select color vibe", items).ok().flatten().unwrap_or(0) {
        1 => ColorMode::Ocean,
        2 => ColorMode::Sunset,
        3 => ColorMode::Matrix,
        _ => ColorMode::Rainbow,
    }
}

pub fn ask_speed() -> f64 {
    let items: &[(&str, &str)] = &[
        ("Normal", "1.0x — standard"),
        ("Slow", "0.5x — chill"),
        ("Fast", "2.0x — double time"),
        ("Ludicrous", "4.0x — plaid"),
    ];

    match arrow_select("Select speed", items).ok().flatten().unwrap_or(0) {
        1 => 0.5,
        2 => 2.0,
        3 => 4.0,
        _ => 1.0,
    }
}
