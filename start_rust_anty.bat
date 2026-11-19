@echo off
chcp 65001 >nul
cd /d "%~dp0"
if exist "target\release\rust-anty.exe" (
    "target\release\rust-anty.exe"
) else (
    cargo run --release
)
