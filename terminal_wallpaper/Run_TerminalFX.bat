@echo off
cd /d "%~dp0"

echo Building TerminalFX release executable...
cargo build --release

if errorlevel 1 (
    echo.
    echo Build failed. Press any key to close.
    pause >nul
    exit /b 1
)

echo Launching TerminalFX...

where wt.exe >nul 2>nul
if %errorlevel%==0 (
    start "" wt.exe -M --title "TerminalFX" "%~dp0target\release\terminal_wallpaper.exe"
) else (
    start "" "%~dp0target\release\terminal_wallpaper.exe"
)