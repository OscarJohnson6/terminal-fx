from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET, DIM
from typing import Optional, List, Dict
import random, math

class ShootingStarMode(ModeBase):
    """
    Diagonal shooting stars across a starry background.

    - Start near the top-left (either just off the left edge or just above the top).
    - Move to the right with a clear downward angle (no almost-horizontal shots).
    - Some are 'near' (bright, large explosions), some 'far' (dimmer, smaller).
    - Number of active stars is a bit random (1–4).
    - Explode only when they leave the screen.
    - Background: subtle twinkling white dots.
    """
    def __init__(self, speed_factor: float, color_provider: ColorProvider, explode_on_land: bool = True):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.explode_on_land = explode_on_land

        self.stars: List[Dict] = []        # {x, y, dx, dy, speed, length, seed, depth}
        self.explosions: List[Dict] = []   # {x, y, vx, vy, life, seed, depth}

        # Star count randomness
        self.min_stars = 1
        self.max_stars = 4

        # Background stars (twinkling)
        self.bg_stars: List[Dict] = []     # {x, y, phase}
        self.bg_dims: Optional[tuple[int, int]] = None

    # ---------- helpers ----------
    def _ensure_bg_stars(self, width: int, height: int):
        """
        Create / recreate background stars whenever terminal size changes.
        Very subtle density to avoid clutter.
        """
        if self.bg_dims == (width, height) and self.bg_stars:
            return

        self.bg_dims = (width, height)
        self.bg_stars = []

        # ~0.5% of cells become stars (tweak if you want more/less)
        count = int(width * height * 0.005)
        for _ in range(count):
            self.bg_stars.append({
                "x": random.randrange(width),
                "y": random.randrange(height),
                "phase": random.uniform(0, 2 * math.pi),
            })

    def spawn_star(self, width: int, height: int):
        # Spawn either from the left side or from the top-left edge.
        spawn_from_top = random.random() < 0.4  # ~40% from top, 60% from left

        if spawn_from_top:
            x = random.uniform(0, max(1, width * 0.3))
            y = random.uniform(-8, 0)  # slightly above the screen
        else:
            x = random.uniform(-10, 0)  # off-screen to the left
            y = random.uniform(-2, height * 0.3)

        # Choose a clearly diagonal angle: 10°–30° downward from horizontal.
        # 0 rad = horizontal right; we want some vertical component.
        min_angle = math.radians(10)
        max_angle = math.radians(30)
        angle = random.uniform(min_angle, max_angle)

        speed = random.uniform(25, 45) * self.speed_factor
        dx = math.cos(angle)
        dy = math.sin(angle)  # > 0 => downward

        length = random.randint(4, 8)

        # depth in [0.4, 1.0]; smaller = farther (dimmer, smaller explosion)
        depth = random.uniform(0.4, 1.0)
        seed = random.randint(0, 10000)

        self.stars.append({
            "x": x,
            "y": y,
            "dx": dx,
            "dy": dy,
            "speed": speed,
            "length": length,
            "seed": seed,
            "depth": depth,
        })

    def spawn_explosion(self, x: float, y: float, seed: int, depth: float):
        """
        Spawn a short, punchy explosion. Near stars (depth ~1) are larger/brighter,
        far stars (depth ~0.4) are smaller/dimmer.
        """
        # Scale particle count & speed by depth to reinforce distance feeling.
        base_num = 30
        num = max(8, int(base_num * depth))

        for _ in range(num):
            angle = random.uniform(0, 2 * math.pi)
            # Near = faster/bigger burst, far = softer
            speed = random.uniform(10, 28) * depth * self.speed_factor
            vx = speed * math.cos(angle)
            vy = speed * math.sin(angle) * 0.6

            # Shorter life for punchy bursts; depth scales it a bit
            life = random.uniform(0.18, 0.5) * (0.7 + 0.6 * depth)

            self.explosions.append({
                "x": x,
                "y": y,
                "vx": vx,
                "vy": vy,
                "life": life,
                "seed": seed,
                "depth": depth,
            })

    # ---------- main update/render ----------
    def update(self, dt: float, width: int, height: int, t_abs: float):
        # Maintain a random-ish number of stars
        if len(self.stars) < self.min_stars:
            self.spawn_star(width, height)
        elif len(self.stars) < self.max_stars and random.random() < 0.02:
            # small chance each frame to add another star if under the max
            self.spawn_star(width, height)

        new_stars: List[Dict] = []
        for star in self.stars:
            # move the star
            star["x"] += star["dx"] * star["speed"] * dt
            star["y"] += star["dy"] * star["speed"] * dt

            # if it just left the visible region on the right or bottom,
            # explode immediately right at the edge
            if star["x"] >= width or star["y"] >= height:
                if self.explode_on_land:
                    # clamp explosion point to last visible cell
                    exp_x = min(max(star["x"], 0), width - 1)
                    exp_y = min(max(star["y"], 0), height - 1)
                    self.spawn_explosion(exp_x, exp_y, star["seed"], star["depth"])
                # star is done
                continue

            # if it somehow goes way off-screen above or left, just drop it
            if star["x"] < -20 or star["y"] < -20:
                continue

            new_stars.append(star)

        self.stars = new_stars

        # Update explosion particles
        new_parts: List[Dict] = []
        for p in self.explosions:
            p["x"] += p["vx"] * dt
            p["y"] += p["vy"] * dt
            p["life"] -= dt
            if p["life"] > 0:
                new_parts.append(p)
        self.explosions = new_parts

    def render(self, width: int, height: int, t_abs: float) -> str:
        self._ensure_bg_stars(width, height)
        buf = [[" " for _ in range(width)] for _ in range(height)]

        # --- background stars (twinkling) ---
        for s in self.bg_stars:
            x = s["x"]
            y = s["y"]
            if 0 <= x < width and 0 <= y < height:
                # 0..1 brightness
                b = (math.sin(t_abs * 0.7 + s["phase"]) + 1) / 2
                if b < 0.15:
                    # occasionally "off" for a blink
                    continue
                # dim vs bright white-ish
                if b < 0.5:
                    col = "\033[38;5;244m"  # gray
                    ch = "."
                else:
                    col = "\033[38;5;250m"  # light white
                    ch = "·"
                if buf[y][x] == " ":
                    buf[y][x] = col + ch + RESET

        # --- shooting stars ---
        for star in self.stars:
            head_x = star["x"]
            head_y = star["y"]
            dx = star["dx"]
            dy = star["dy"]
            length = star["length"]
            depth = star["depth"]
            seed = star["seed"]

            base_col = self.color_provider.get(t_abs, seed)
            # Farther stars are dimmer overall
            head_prefix = "" if depth > 0.75 else DIM

            for i in range(length):
                x = int(head_x - dx * i)
                y = int(head_y - dy * i)
                if 0 <= x < width and 0 <= y < height:
                    if i == 0:
                        ch = "@"
                        col = head_prefix + base_col
                    else:
                        col = "\033[38;5;240m" if depth > 0.7 else "\033[38;5;238m"
                        ch = "*" if i < length // 2 else "."
                    buf[y][x] = col + ch + RESET

        # --- explosions ---
        for p in self.explosions:
            x = int(p["x"])
            y = int(p["y"])
            if 0 <= x < width and 0 <= y < height:
                base_col = self.color_provider.get(t_abs, p["seed"])
                # Farther explosions are dimmer
                col = base_col if p["depth"] > 0.75 else DIM + base_col
                ch = random.choice("*+x0123456789abcdef")
                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)