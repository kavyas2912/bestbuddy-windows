@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0RUN_WINDOWS.ps1"
endlocal & exit /b %ERRORLEVEL%
