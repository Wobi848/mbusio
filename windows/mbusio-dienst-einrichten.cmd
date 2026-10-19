@echo off
rem mbusio als Windows-Dienst einrichten oder aktualisieren (Konzept Hintergrund). Neben die mbusio-.exe legen und doppelklicken.
setlocal
net session >nul 2>&1
if errorlevel 1 (
  rem ohne Adminrechte: sich selbst als Administrator neu starten (Windows fragt nach)
  powershell -NoProfile -Command "Start-Process -FilePath '%~f0' -Verb RunAs"
  exit /b
)
cd /d "%~dp0"
set "EXE="
for %%f in (mbusio*.exe) do set "EXE=%%f"
if not defined EXE (
  echo Keine mbusio-.exe in diesem Ordner gefunden / no mbusio .exe found in this folder.
  pause
  exit /b 1
)
echo mbusio als Windows-Dienst einrichten / set up mbusio as a Windows service
echo.
set "OPTS="
set /p LAN="Oberflaeche auch aus dem Netzwerk erreichbar? / reachable from the network? (j/n) "
if /i "%LAN%"=="j" set /p TOKEN="Zugriffsschluessel / access key: "
if /i "%LAN%"=="j" set "OPTS=--lan --token "%TOKEN%""
"%EXE%" --install-service %OPTS%
