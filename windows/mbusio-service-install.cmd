@echo off
rem Set up or update mbusio as a Windows service. Put next to the mbusio .exe and double-click.
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
echo Set up mbusio as a Windows service
echo.
set "OPTS="
set /p LAN="Reachable from the network? (y/n) "
if /i "%LAN%"=="y" set /p TOKEN="Access key: "
if /i "%LAN%"=="y" set "OPTS=--lan --token "%TOKEN%""
"%EXE%" --install-service %OPTS%
