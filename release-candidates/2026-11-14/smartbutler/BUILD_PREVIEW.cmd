@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\BUILD_STORE_MSIX_WINDOWS.ps1" -Mode Preview %*
set "RC=%ERRORLEVEL%"
if not "%RC%"=="0" echo [SmartButler] PREVIEW build failed: %RC%
endlocal & exit /b %RC%
