@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\INSTALL_PREVIEW_WINDOWS.ps1"
set "RC=%ERRORLEVEL%"
if not "%RC%"=="0" (
  echo [SmartButler] Preview installation failed: %RC%
  pause
)
endlocal & exit /b %RC%
