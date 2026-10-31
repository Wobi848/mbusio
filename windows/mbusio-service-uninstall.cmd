@echo off
rem Remove the mbusio Windows service; program and data stay. Put next to the mbusio .exe and double-click.
setlocal
net session >nul 2>&1
if errorlevel 1 (
  rem without admin rights: restart as administrator (Windows asks)
  powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
  exit /b
)
cd /d "%~dp0"
set "EXE="
for %%f in (mbusio*.exe) do set "EXE=%%f"
if not defined EXE (
  echo No mbusio .exe found in this folder.
  pause
  exit /b 1
)
"%EXE%" --uninstall-service
