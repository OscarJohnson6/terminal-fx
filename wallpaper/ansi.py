# ===== Terminal ANSI Escape Codes =====

# Cursor + screen control
HIDE_CURSOR = "\033[?25l"
SHOW_CURSOR = "\033[?25h"
CLEAR = "\033[2J"
HOME = "\033[H"

# Text styling
RESET  = "\033[0m"
BRIGHT = "\033[1m"
DIM    = "\033[2m"
ITALIC = "\033[3m"
UNDER  = "\033[4m"

# Foreground colors
FG_BLACK   = "\033[30m"
FG_RED     = "\033[31m"
FG_GREEN   = "\033[32m"
FG_YELLOW  = "\033[33m"
FG_BLUE    = "\033[34m"
FG_MAGENTA = "\033[35m"
FG_CYAN    = "\033[36m"
FG_WHITE   = "\033[37m"

# Bright foreground colors
FG_BRIGHT_BLACK   = "\033[90m"
FG_BRIGHT_RED     = "\033[91m"
FG_BRIGHT_GREEN   = "\033[92m"
FG_BRIGHT_YELLOW  = "\033[93m"
FG_BRIGHT_BLUE    = "\033[94m"
FG_BRIGHT_MAGENTA = "\033[95m"
FG_BRIGHT_CYAN    = "\033[96m"
FG_BRIGHT_WHITE   = "\033[97m"

# Background colors (optional, useful for UI elements)
BG_BLACK   = "\033[40m"
BG_RED     = "\033[41m"
BG_GREEN   = "\033[42m"
BG_YELLOW  = "\033[43m"
BG_BLUE    = "\033[44m"
BG_MAGENTA = "\033[45m"
BG_CYAN    = "\033[46m"
BG_WHITE   = "\033[47m"

# Frame rate
FPS = 60

# Windows ANSI enabling
import os
def enable_ansi_on_windows():
    if os.name == "nt":
        os.system("")  # Enables ANSI in CMD/PowerShell
