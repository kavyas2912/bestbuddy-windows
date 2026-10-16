@echo off
setlocal
cd /d "%~dp0..\apps\vce-desktop"
if not exist package.json (echo ERROR: package.json not found.& exit /b 1)
call npm ci --ignore-scripts --include=optional --no-audit --no-fund || exit /b 1
node node_modules\esbuild\install.js || exit /b 1
call node_modules\.bin\vite.cmd --version || exit /b 1
endlocal & exit /b 0
