from typing import Optional

# ----------------- Color codes and palettes -----------------
# A rainbow-like path through 256-color palette
RAINBOW_CODES = [
    196, 202, 208, 214, 220, 226,   # reds -> yellows
    190, 118, 46,                   # greens
    51, 39, 21,                     # cyans/blues
    57, 93, 129, 165,               # purples/pinks
]

# Themed color paths (256-color indexes).
THEME_PATHS = {
    "rainbow_slow": [
        196, 202, 208, 214, 220, 226,
        190, 154, 118, 82, 46,
        47, 48, 49, 51,
        45, 39, 33, 27, 21,
        54, 55, 56, 93, 129, 165, 201,
        198, 197, 196,
    ],
    "ocean": [
        17, 18, 19, 20, 25, 26, 31, 32,
        37, 38, 44, 45, 51, 50, 39, 30,
    ],
    "fire": [
        52, 88, 124, 160, 196, 202, 208, 214,
        220, 226, 220, 214, 208, 202, 196,
    ],
    "forest": [
        22, 28, 34, 40, 46, 82, 112, 148, 190, 154, 70, 64,
    ],
    "dusk": [
        54, 55, 56, 93, 129, 165, 201, 200, 199, 198,
    ],
}
STATIC_PALETTES = {
    "green": [34, 40, 46],
    "cyan": [37, 44, 51],
    "blue": [20, 21, 27],
    "magenta": [163, 165, 201],
    "red": [160, 196, 202],
    "white": [250, 255],
}

def color_256(idx: int) -> str:
    return f"\033[38;5;{idx}m"

# ----------------- Color provider -----------------
class ColorProvider:
    # Central color logic for all modes.
    def __init__(
        self,
        mode_name: str,
        static_name: Optional[str],
        spectrum_name: Optional[str],
        color_speed_factor: float = 1.0,
    ):
        self.mode_name = mode_name  # 'static' / 'rainbow' / 'spectrum'
        self.static_name = static_name
        self.spectrum_name = spectrum_name
        self.color_speed_factor = color_speed_factor

        if mode_name == "static":
            self.static_palette = STATIC_PALETTES.get(static_name, [46]) # type: ignore
        else:
            self.static_palette = None

        if mode_name == "spectrum":
            self.theme_codes = THEME_PATHS.get(spectrum_name, RAINBOW_CODES) # type: ignore
        else:
            self.theme_codes = None

    def rainbow_color(self, t: float, x: int, base_speed: float = 0.1) -> str:
        speed = base_speed * self.color_speed_factor
        f = (t * speed + x * 0.1) % len(RAINBOW_CODES)
        idx = RAINBOW_CODES[int(f)]
        return color_256(idx)

    def static_color(self, t: float, x: int) -> str:
        palette = self.static_palette or [46]
        idx = palette[x % len(palette)]
        return color_256(idx)

    def spectrum_color(self, t: float, x: int, base_speed: float = 0.4) -> str:
        codes = self.theme_codes or RAINBOW_CODES
        n = len(codes)
        if n == 0:
            return color_256(46)
        speed = base_speed * self.color_speed_factor
        f = (t * speed + x * 0.07) % n
        idx = codes[int(f)]
        return color_256(idx)

    def get(self, t: float, x: int) -> str:
        if self.mode_name == "static":
            return self.static_color(t, x)
        if self.mode_name == "rainbow":
            return self.rainbow_color(t, x)
        if self.mode_name == "spectrum":
            return self.spectrum_color(t, x)
        return color_256(46)