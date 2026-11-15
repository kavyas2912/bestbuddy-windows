@echo off
setlocal
cd /d "%~dp0apps\vce-desktop"
echo SMART BUTLER - ORANGUTAN EDITION - SB-20261021-R8-SHARED-THEME
echo Building THIS extracted source. Preview: http://127.0.0.1:1433/
call npm.cmd ci --no-audit --no-fund
if errorlevel 1 goto failed
call npm.cmd run build
if errorlevel 1 goto failed
node node_modules\vite\bin\vite.js preview --host 127.0.0.1 --port 1433 --strictPort
exit /b %ERRORLEVEL%
:failed
echo Smart Butler preview did not start. Read the error above.
pause
exit /b 1
