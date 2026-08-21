@echo off
setlocal EnableExtensions
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"

if /I "%~1"=="dev" goto dev
if /I "%~1"=="build" goto build
if /I "%~1"=="preview" goto preview
goto usage

:dev
echo [run] dev — compiling HTML and launching with hot-reload...
cargo run -p app
goto :eof

:build
echo [run] build — optimized release build...
cargo build --release -p app
if errorlevel 1 exit /b 1
echo [run] built target\release\app.exe
goto :eof

:preview
if not exist "target\release\app.exe" (
  echo [run] no release build found. Run: run build
  exit /b 1
)
echo [run] preview — launching release build...
start "" "target\release\app.exe"
goto :eof

:usage
echo Usage: run [dev^|build^|preview]
echo   dev     - compile root/*.html and run with HMR hot-reload (debug)
echo   build   - optimized release build (target\release\app.exe)
echo   preview - launch the release exe
exit /b 1
