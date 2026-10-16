@echo off
setlocal
cd /d "%~dp0.."
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0TEST_WINDOWS.ps1"
set "RC=%ERRORLEVEL%"
endlocal & exit /b %RC%
