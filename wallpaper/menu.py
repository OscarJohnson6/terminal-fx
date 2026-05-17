from .color import ColorProvider, STATIC_PALETTES, THEME_PATHS
from .ansi import (
    RESET, BRIGHT as BOLD, DIM,
    FG_BRIGHT_CYAN as FG_TITLE,
    FG_BRIGHT_YELLOW as FG_INDEX,
    FG_WHITE as FG_DESC,
    FG_BRIGHT_GREEN as FG_INPUT,
)
import random

# ----------------- Menu helpers -----------------
def ask_mode() -> str:
    modes = [
        ("1",  "scroll",     "fake commands + logs"),
        ("2",  "wave-full",  "full-screen wave"),
        ("3",  "wave-line",  "slithering line"),
        ("4",  "matrix",     "falling digits"),
        ("5",  "fireworks",  "vertical rockets"),
        ("6",  "bursts",     "random explosions"),
        ("7",  "stars",      "diagonal shooting stars"),
        ("8",  "orbs",       "colliding orbs"),
        ("9",  "bounce",     "gravity balls"),
        ("10", "pendulum",   "double-pendulum trace"),
        ("11", "pulse",      "expanding rings"),
        ("12", "worm-field", "flowing worm trails"),
        ("13", "orbitals",  "moving orbiting particles"),
        ("14", "sand",      "falling sand piles"),
        ("15", "nebula",   "swirling space clouds"),
        ("16", "fire",     "campfire / torch flames"),
        ("17", "village",  "tiny villages growing & fighting"),
        ("18", "meteors", "meteor shower + fragments"),
    ]

    print()
    print(FG_TITLE + BOLD + "=== Select Animation Mode ===" + RESET)
    print(DIM + "Type a number, a mode name, or 'q' to quit.\n" + RESET)

    for idx, name, desc in modes:
        print(f"  {FG_INDEX}{idx:>2}){RESET} {BOLD}{name:<11}{RESET} "
              f"{DIM}({desc}){RESET}")

    # Map all valid inputs to canonical mode names
    alias_map = {
        # 1: scroll
        "1": "scroll", "scroll": "scroll", "s": "scroll",
        # 2: wave_full
        "2": "wave_full", "wave-full": "wave_full", "wf": "wave_full",
        # 3: wave_line
        "3": "wave_line", "wave-line": "wave_line", "wl": "wave_line", "line": "wave_line",
        # 4: matrix
        "4": "matrix", "m": "matrix", "matrix": "matrix",
        # 5: fireworks
        "5": "fireworks", "fw": "fireworks", "firework": "fireworks", "fireworks": "fireworks",
        # 6: bursts
        "6": "bursts", "burst": "bursts", "b": "bursts",
        # 7: stars
        "7": "stars", "star": "stars",
        # 8: orbs
        "8": "orbs", "orb": "orbs",
        # 9: bounce
        "9": "bounce", "bounce": "bounce", "ball": "bounce", "balls": "bounce",
        # 10: pendulum
        "10": "pendulum", "pendulum": "pendulum", "pend": "pendulum",
        # 11: pulse
        "11": "pulse", "pulse": "pulse", "rings": "pulse", "ring": "pulse",
        # 12: worm-field
        "12": "worm", "worm": "worm", "worms": "worm", "worm-field": "worm",
        # 13: orbitals
        "13": "orbitals", "orbitals": "orbitals", "orbit": "orbitals",
        # 14: sand
        "14": "sand", "sand": "sand", "sandfall": "sand",
        # 15: nebula
        "15": "nebula", "nebula": "nebula",
        # 16: fire
        "16": "fire", "campfire": "fire", "flame": "fire",
        # 17: village
        "17": "village", "villages": "village", "worldbox": "village",
        # 18: meteor shower
        "18": "meteors", "meteors": "meteors", "meteor": "meteors", "meteor-shower": "meteors", "shower": "meteors",
    }

    while True:
        choice = input(FG_INPUT + "> " + RESET).strip().lower()

        if not choice:
            continue

        # Let user quit from the mode menu as well
        if choice in ("q", "quit", "exit"):
            return "q"

        mode = alias_map.get(choice)
        if mode is not None:
            return mode

        print("Please type 1–12, a mode name, or 'q' to quit.")


def ask_speed_factor() -> float:
    print("\nOptional: visual speed multiplier (blank = 1.0)")
    print("  0.5 = slower, 1 = normal, 2 = faster, etc.")
    print("  r   = random speed in a nice range")
    s = input("visual speed> ").strip().lower()
    if not s:
        return 1.0

    if s in ("r", "random"):
        f = random.uniform(0.7, 1.8)
        print(f"Random visual speed selected: {f:.2f}x")
        return f

    try:
        f = float(s)
    except ValueError:
        print("Invalid number, using 1.0.")
        return 1.0
    if f <= 0:
        print("Speed must be positive. Using 1.0 instead.")
        return 1.0
    if f > 10:
        print("That is extremely fast; clamping to 10x.")
        return 10.0
    return f


def ask_color_speed_factor() -> float:
    print("\nOptional: COLOR speed multiplier (blank = 1.0)")
    print("  0   = almost static colors")
    print("  0.5 = slower color changes")
    print("  1   = normal")
    print("  2   = faster, etc.")
    s = input("color speed> ").strip()
    if not s:
        return 1.0
    try:
        f = float(s)
    except ValueError:
        print("Invalid number, using 1.0.")
        return 1.0
    if f < 0:
        print("Color speed cannot be negative. Using 0 (static-ish).")
        return 0.0
    if f > 10:
        print("Very fast color cycling; clamping to 10x.")
        return 10.0
    return f


def ask_color_mode(color_speed_factor: float) -> ColorProvider:
    print("\nColor mode:")
    print("  1) static   (predefined solid palettes)")
    print("  2) rainbow  (moving rainbow)")
    print("  3) spectrum (themes: ocean, fire, forest, dusk, rainbow_slow)")
    while True:
        choice = input("color mode> ").strip().lower()
        if choice in ("1", "static"):
            return ask_static_palette(color_speed_factor)
        if choice in ("2", "rainbow"):
            return ColorProvider("rainbow", None, None, color_speed_factor)
        if choice in ("3", "spectrum"):
            return ask_spectrum(color_speed_factor)
        print("Please type 1/2/3 or static/rainbow/spectrum.")


def ask_static_palette(color_speed_factor: float) -> ColorProvider:
    print("\nStatic palettes:")
    for name in STATIC_PALETTES:
        print(f"  - {name}")
    while True:
        name = input("palette name> ").strip().lower()
        if name in STATIC_PALETTES:
            return ColorProvider("static", name, None, color_speed_factor)
        print("Unknown palette. Available:", ", ".join(STATIC_PALETTES.keys()))


def ask_spectrum(color_speed_factor: float) -> ColorProvider:
    print("\nSpectrum themes:")
    for name in THEME_PATHS:
        print(f"  - {name}")
    while True:
        name = input("spectrum name> ").strip().lower()
        if name in THEME_PATHS:
            return ColorProvider("spectrum", None, name, color_speed_factor)
        print("Unknown spectrum. Available:", ", ".join(THEME_PATHS.keys()))
