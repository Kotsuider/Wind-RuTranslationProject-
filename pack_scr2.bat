@echo off
chcp 65001 >nul
title Pack scr2.paz
cd /d %~dp0

if not exist wind_paz.exe (
    echo [ERROR] wind_paz.exe not found in %~dp0!
    pause
    exit /b 1
)

if not exist unpacked_scr2 (
    echo [ERROR] Folder unpacked_scr2 not found!
    pause
    exit /b 1
)

if exist scr2.paz (
    if not exist scr2.paz.bak (
        echo [*] Backup: scr2.paz -^> scr2.paz.bak
        copy /y scr2.paz scr2.paz.bak >nul
    )
)

echo [*] Packing scr2.paz...
wind_paz.exe pack unpacked_scr2 scr2.paz

if %ERRORLEVEL% EQU 0 (
    echo.
    echo [SUCCESS] scr2.paz packed successfully!
) else (
    echo.
    echo [ERROR] Packing failed!
)

pause
