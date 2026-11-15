@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\BUILD_ANDROID_ENGINEERING.ps1" %*
set "RC=%ERRORLEVEL%"
if not "%RC%"=="0" echo Android APK build failed. Check the error above.
endlocal & exit /b %RC%
