@echo off
setlocal
cd /d "%~dp0.."
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0SETUP_WINDOWS.ps1"
set "RC=%ERRORLEVEL%"
echo.
if not "%RC%"=="0" echo SETUP FAILED with code %RC%.
endlocal & exit /b %RC%
