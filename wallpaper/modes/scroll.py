from ..base_mode import ModeBase
from ..color import ColorProvider
from ..ansi import RESET, DIM
from typing import List
from datetime import datetime
import random

# ----------------- Fake content generators -----------------
COMMANDS = [
    "ls -la",
    "ps aux | head",
    "python3 server.py",
    "cargo build --release",
    "go test ./...",
    "git status",
    "npm run dev",
    "docker ps",
]
BRIGHT = "\033[1m"
EXTS = [".py", ".rs", ".go", ".js", ".log", ".db", ".bin", ".txt", ".md"]
WORDS = ["core", "engine", "cache", "index", "segment", "stream", "kernel", "shader"]

def rand_file():
    return (
        random.choice(WORDS)
        + "_"
        + random.choice(WORDS)
        + "_"
        + str(random.randint(0, 9999)).zfill(4)
        + random.choice(EXTS)
    )

def rand_log():
    ts = datetime.now().strftime("%H:%M:%S")
    lvl = random.choice(["INFO", "WARN", "DEBUG", "TRACE", "ERROR"])
    return f"[{ts}] {lvl:<5} {rand_file()} updated"

# ----------------- Modes -----------------
class ScrollMode(ModeBase):
    def __init__(self, speed_factor: float, color_provider: ColorProvider):
        self.speed_factor = speed_factor
        self.color_provider = color_provider
        self.lines: List[str] = []
        self.time_since_line = 0.0
        self.base_interval = 0.18

    def update(self, dt: float, width: int, height: int, t_abs: float):
        self.time_since_line += dt
        interval = self.base_interval / max(0.1, self.speed_factor)
        if self.time_since_line < interval:
            return
        self.time_since_line = 0.0

        cmd = random.choice(COMMANDS)
        prompt_color = self.color_provider.get(t_abs, len(self.lines))
        prompt = f"{prompt_color}{BRIGHT}user@host{RESET}:{prompt_color}~$ {RESET}"
        block = [prompt + cmd]

        for _ in range(random.randint(1, 4)):
            if random.random() < 0.5:
                perms = "-rw-r--r--"
                size = random.randint(500, 200000)
                line = (
                    f"{DIM}{perms} 1 user users {size:>8} "
                    f"{datetime.now().strftime('%b %d %H:%M')} {rand_file()}{RESET}"
                )
                block.append(line)
            else:
                col = self.color_provider.get(t_abs, random.randint(0, 1000))
                block.append(col + rand_log() + RESET)

        self.lines.extend(block)
        max_lines = height
        if len(self.lines) > max_lines:
            self.lines = self.lines[-max_lines:]

    def render(self, width: int, height: int, t_abs: float) -> str:
        buf = self.lines[-height:]
        while len(buf) < height:
            buf.insert(0, "")
        return "\n".join(line[:width].ljust(width) for line in buf)