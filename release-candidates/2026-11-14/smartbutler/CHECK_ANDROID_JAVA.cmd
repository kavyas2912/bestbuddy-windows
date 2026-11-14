@echo off
setlocal
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\BUILD_ANDROID_ENGINEERING.ps1" -CheckJavaOnly %*
set "RC=%ERRORLEVEL%"
endlocal & exit /b %RC%
