from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET, DIM
from typing import Optional, List, Dict
import random, math


class MeteorShowerMode(ModeBase):
    """
    Meteor shower wallpaper mode (built from your ShootingStarMode idea).

    Features:
    - Parallax background stars (2 layers) with gentle drift + twinkle
    - Meteors spawn from top/left (mostly), sometimes from top/right for variety
    - Trails fade more smoothly than classic shootingstars
    - Occasional fragmentation mid-flight into 2 smaller meteors
    - Edge explosions like your ShootingStarMode, depth-scaled

    Tunables are near __init__.
    """

    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider

        # Active meteors + explosion particles
        self.meteors: List[Dict] = []      # {x,y, dx,dy, speed, length, seed, depth, frag, frag_timer}
        self.parts: List[Dict] = []        # {x,y, vx,vy, life, seed, depth}

        # Meteors: keep a lively count
        self.min_meteors = 1
        self.max_meteors = 6

        # Background starfield (parallax)
        self.bg_dims: Optional[tuple[int, int]] = None
        self.bg_near: List[Dict] = []      # brighter, faster drift
        self.bg_far: List[Dict] = []       # dimmer, slower drift

        # Drift controls
        self.drift_x_near = 0.60
        self.drift_x_far  = 0.20

    # ---------------- Background ----------------
    def _ensure_bg(self, width: int, height: int):
        if self.bg_dims == (width, height) and (self.bg_near or self.bg_far):
            return

        self.bg_dims = (width, height)
        self.bg_near = []
        self.bg_far = []

        # Density tuned to stay subtle in large terminals
        # Far: ~0.35% of cells, Near: ~0.15% of cells
        far_count = int(width * height * 0.0035)
        near_count = int(width * height * 0.0015)

        for _ in range(far_count):
            self.bg_far.append({
                "x": random.uniform(0, width - 1),
                "y": random.uniform(0, height - 1),
                "phase": random.uniform(0, 2 * math.pi),
            })

        for _ in range(near_count):
            self.bg_near.append({
                "x": random.uniform(0, width - 1),
                "y": random.uniform(0, height - 1),
                "phase": random.uniform(0, 2 * math.pi),
            })

    def _update_bg(self, dt: float, width: int, height: int, t_abs: float):
        # gentle rightward drift (wrap around)
        sx_near = self.drift_x_near * self.speed_factor
        sx_far  = self.drift_x_far  * self.speed_factor

        for s in self.bg_far:
            s["x"] += sx_far * dt
            if s["x"] >= width:
                s["x"] -= width

        for s in self.bg_near:
            s["x"] += sx_near * dt
            if s["x"] >= width:
                s["x"] -= width

    # ---------------- Meteors ----------------
    def _spawn_meteor(self, width: int, height: int):
        # Mostly from top/left, sometimes from top/right for variety
        r = random.random()
        if r < 0.55:
            # from left
            x = random.uniform(-12, 0)
            y = random.uniform(-2, height * 0.35)
            base_dir = 0.0
        elif r < 0.90:
            # from top
            x = random.uniform(0, width * 0.7)
            y = random.uniform(-10, 0)
            base_dir = 0.0
        else:
            # from top-right (rare, looks cool)
            x = random.uniform(width * 0.7, width + 12)
            y = random.uniform(-10, 0)
            base_dir = math.pi  # going left-ish

        # Angle: keep a clear downward slant (like your 10°–30° idea)
        min_a = math.radians(12)
        max_a = math.radians(34)
        a = random.uniform(min_a, max_a)

        # Turn angle depending on spawn side
        if base_dir == 0.0:
            dx = math.cos(a)
            dy = math.sin(a)
        else:
            # coming from right: dx negative
            dx = -math.cos(a)
            dy = math.sin(a)

        # Speed and trail length
        speed = random.uniform(22, 55) * self.speed_factor
        length = random.randint(6, 12)

        # depth: far=dim + smaller, near=bright + chunkier
        depth = random.uniform(0.35, 1.0)
        seed = random.randint(0, 999999)

        # fragmentation: near meteors fragment more often
        frag_chance = 0.12 + 0.18 * depth  # ~0.18..0.30
        frag_timer = random.uniform(0.25, 1.10)  # time until it *may* fragment

        self.meteors.append({
            "x": x, "y": y,
            "dx": dx, "dy": dy,
            "speed": speed,
            "length": length,
            "seed": seed,
            "depth": depth,
            "frag": frag_chance,
            "frag_timer": frag_timer,
        })

    def _spawn_explosion(self, x: float, y: float, seed: int, depth: float):
        # Similar to your star explosions: short and punchy, depth-scaled. :contentReference[oaicite:1]{index=1}
        base_num = 36
        num = max(10, int(base_num * (0.55 + depth)))

        for _ in range(num):
            ang = random.uniform(0, 2 * math.pi)
            spd = random.uniform(8, 26) * depth * self.speed_factor
            vx = spd * math.cos(ang)
            vy = spd * math.sin(ang) * 0.6
            life = random.uniform(0.14, 0.55) * (0.65 + 0.7 * depth)
            self.parts.append({
                "x": x, "y": y,
                "vx": vx, "vy": vy,
                "life": life,
                "seed": seed,
                "depth": depth,
            })

    def _fragment(self, m: Dict):
        # Split into two smaller meteors with slight angle divergence
        x, y = m["x"], m["y"]
        dx, dy = m["dx"], m["dy"]

        # Normalize (just in case)
        mag = max(1e-6, math.sqrt(dx * dx + dy * dy))
        dx /= mag
        dy /= mag

        # Two diverging angle offsets
        off = random.uniform(math.radians(6), math.radians(14))
        for sign in (-1, 1):
            # rotate direction vector slightly
            c = math.cos(off * sign)
            s = math.sin(off * sign)
            ndx = dx * c - dy * s
            ndy = dx * s + dy * c

            child_depth = max(0.25, m["depth"] * random.uniform(0.75, 0.95))
            child_speed = m["speed"] * random.uniform(0.75, 0.92)
            child_len = max(4, int(m["length"] * random.uniform(0.55, 0.8)))

            self.meteors.append({
                "x": x, "y": y,
                "dx": ndx, "dy": ndy,
                "speed": child_speed,
                "length": child_len,
                "seed": m["seed"] + random.randint(1, 999),
                "depth": child_depth,
                "frag": 0.0,              # children do not fragment again (keeps chaos under control)
                "frag_timer": 999.0,
            })

        # pop a tiny sparkle where it breaks
        self._spawn_explosion(x, y, m["seed"], m["depth"] * 0.45)

    # ---------------- Main loop ----------------
    def update(self, dt: float, width: int, height: int, t_abs: float):
        self._ensure_bg(width, height)
        self._update_bg(dt, width, height, t_abs)

        # Maintain a lively meteor count
        if len(self.meteors) < self.min_meteors:
            self._spawn_meteor(width, height)
        elif len(self.meteors) < self.max_meteors and random.random() < 0.035:
            self._spawn_meteor(width, height)

        # Update meteors
        new_meteors: List[Dict] = []
        for m in self.meteors:
            m["frag_timer"] -= dt

            m["x"] += m["dx"] * m["speed"] * dt
            m["y"] += m["dy"] * m["speed"] * dt

            # fragmentation (mid-flight)
            if m["frag_timer"] <= 0 and m["frag"] > 0 and random.random() < m["frag"]:
                self._fragment(m)
                continue

            # If it leaves screen, explode at edge and die
            left = (m["x"] < -20)
            right = (m["x"] >= width)
            bottom = (m["y"] >= height)
            top = (m["y"] < -20)

            if right or bottom or left or top:
                # only explode if it *exited* through visible-ish edges
                if right or bottom:
                    exp_x = min(max(m["x"], 0), width - 1)
                    exp_y = min(max(m["y"], 0), height - 1)
                    self._spawn_explosion(exp_x, exp_y, m["seed"], m["depth"])
                continue

            new_meteors.append(m)

        self.meteors = new_meteors

        # Update particles
        new_parts: List[Dict] = []
        for p in self.parts:
            p["x"] += p["vx"] * dt
            p["y"] += p["vy"] * dt
            p["life"] -= dt
            if p["life"] > 0:
                new_parts.append(p)
        self.parts = new_parts

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = [[" " for _ in range(width)] for _ in range(height)]

        # --- Starfield: far layer (dimmer) ---
        for s in self.bg_far:
            x = int(s["x"]) % width
            y = int(s["y"]) % height
            b = (math.sin(t_abs * 0.55 + s["phase"]) + 1.0) / 2.0
            if b < 0.20:
                continue
            col = "\033[38;5;240m" if b < 0.55 else "\033[38;5;245m"
            ch = "." if b < 0.55 else "·"
            if buf[y][x] == " ":
                buf[y][x] = col + ch + RESET

        # --- Starfield: near layer (slightly brighter) ---
        for s in self.bg_near:
            x = int(s["x"]) % width
            y = int(s["y"]) % height
            b = (math.sin(t_abs * 0.75 + s["phase"]) + 1.0) / 2.0
            if b < 0.25:
                continue
            col = "\033[38;5;248m" if b < 0.60 else "\033[38;5;252m"
            ch = "·" if b < 0.60 else "*"
            if buf[y][x] == " ":
                buf[y][x] = col + ch + RESET

        # --- Meteors ---
        for m in self.meteors:
            head_x = m["x"]
            head_y = m["y"]
            dx = m["dx"]
            dy = m["dy"]
            length = m["length"]
            depth = m["depth"]
            seed = m["seed"]

            base_col = self.color_provider.get(t_abs, seed)  # ColorProvider.get(t,x) :contentReference[oaicite:2]{index=2}
            head_dim = "" if depth > 0.75 else DIM

            # Smooth fade: head '@', then 'o', then '*', then '.'
            for i in range(length):
                x = int(head_x - dx * i)
                y = int(head_y - dy * i)
                if 0 <= x < width and 0 <= y < height:
                    if i == 0:
                        ch = "@"
                        col = head_dim + base_col
                    elif i == 1:
                        ch = "o"
                        col = head_dim + base_col
                    elif i < length // 2:
                        ch = "*"
                        col = "\033[38;5;244m" if depth > 0.7 else "\033[38;5;242m"
                    else:
                        ch = "."
                        col = "\033[38;5;240m" if depth > 0.7 else "\033[38;5;238m"
                    buf[y][x] = col + ch + RESET

        # --- Explosions ---
        for p in self.parts:
            x = int(p["x"])
            y = int(p["y"])
            if 0 <= x < width and 0 <= y < height:
                base_col = self.color_provider.get(t_abs, p["seed"])
                col = base_col if p["depth"] > 0.75 else (DIM + base_col)
                ch = random.choice("*+x0123456789abcdef")
                buf[y][x] = col + ch + RESET

        return "\n".join("".join(row) for row in buf)
