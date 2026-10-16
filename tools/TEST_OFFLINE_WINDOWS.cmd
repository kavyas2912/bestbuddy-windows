@echo off
setlocal
cd /d "%~dp0.."
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0TEST_OFFLINE_WINDOWS.ps1"
set "RC=%ERRORLEVEL%"
if not "%RC%"=="0" echo Offline tests failed with code %RC%.
endlocal & exit /b %RC%
