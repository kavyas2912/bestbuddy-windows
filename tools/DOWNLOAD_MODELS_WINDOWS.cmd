@echo off
setlocal
python "%~dp0prepare_voice_models.py" --target-root "%~dp0.." --cache "%~dp0cache\voice-models" %*
endlocal & exit /b %ERRORLEVEL%
