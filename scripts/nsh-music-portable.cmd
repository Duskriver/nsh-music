@echo off
setlocal
set "BASE_DIR=%~dp0"
set "DATA_DIR=%BASE_DIR%data"
if not exist "%DATA_DIR%\roaming" mkdir "%DATA_DIR%\roaming"
if not exist "%DATA_DIR%\local" mkdir "%DATA_DIR%\local"
if not exist "%DATA_DIR%\session" mkdir "%DATA_DIR%\session"
set "APPDATA=%DATA_DIR%\roaming"
set "LOCALAPPDATA=%DATA_DIR%\local"
set "NSH_MUSIC_DATA_DIR=%DATA_DIR%\session"
start "" "%BASE_DIR%nsh-music.exe" %*
