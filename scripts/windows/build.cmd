@echo off
setlocal EnableExtensions
rem ----------------------------------------------------------------------------------------
rem  Builds CargoFrame on Windows: the program and the installer.
rem
rem    scripts\windows\build.cmd             program + installer
rem    scripts\windows\build.cmd program     program only (no installer)
rem    scripts\windows\build.cmd install     program + installer, then opens the installer
rem
rem  Add  fast     to skip reinstalling the web packages (when ui\package-lock.json is unchanged)
rem  Add  nopause  to close without waiting for a key (VS Code tasks and CI use this)
rem
rem  This is a cmd script on purpose. In PowerShell, "npm" runs npm.ps1, which Windows blocks
rem  when script running is disabled. Here every tool is started as its .cmd file instead.
rem ----------------------------------------------------------------------------------------
set "MODE=installer"
set "FAST="
set "NOPAUSE="
set "RC=0"
set "BADOPTION="

for %%A in (%*) do call :option "%%~A"
if defined BADOPTION goto :usage

cd /d "%~dp0..\.."
echo CargoFrame build in %CD%
echo.

where cargo >nul 2>&1
if errorlevel 1 goto :nocargo
where node >nul 2>&1
if errorlevel 1 goto :nonode

if defined FAST if exist "ui\node_modules" goto :build
echo [1/2] Installing the web packages ...
pushd ui
call npm.cmd ci --no-audit --no-fund
if errorlevel 1 goto :failpop
popd

:build
if /i "%MODE%"=="program" goto :buildprogram
echo [2/2] Building the program and the installer. The first time takes several minutes ...
pushd app
call ..\ui\node_modules\.bin\tauri.cmd build --bundles nsis
if errorlevel 1 goto :failpop
popd
goto :built

:buildprogram
echo [2/2] Building the program ...
pushd app
call ..\ui\node_modules\.bin\tauri.cmd build --no-bundle
if errorlevel 1 goto :failpop
popd

:built
if not exist "target\release\cargoframe.exe" goto :fail
echo.
echo Done.
echo   Program:   %CD%\target\release\cargoframe.exe
if /i "%MODE%"=="program" goto :end

set "SETUP="
for /f "delims=" %%F in ('dir /b /o-d "target\release\bundle\nsis\*-setup.exe" 2^>nul') do if not defined SETUP set "SETUP=%CD%\target\release\bundle\nsis\%%F"
if not defined SETUP goto :fail
echo   Installer: %SETUP%
if /i "%MODE%"=="install" start "" "%SETUP%"
goto :end

:option
if /i "%~1"=="program"   ( set "MODE=program"   & exit /b 0 )
if /i "%~1"=="installer" ( set "MODE=installer" & exit /b 0 )
if /i "%~1"=="install"   ( set "MODE=install"   & exit /b 0 )
if /i "%~1"=="fast"      ( set "FAST=1"         & exit /b 0 )
if /i "%~1"=="nopause"   ( set "NOPAUSE=1"      & exit /b 0 )
echo Unknown option: %~1
set "BADOPTION=1"
exit /b 0

:usage
echo.
echo Usage: build.cmd [program ^| installer ^| install] [fast] [nopause]
set "RC=2"
goto :end

:nocargo
echo.
echo Rust was not found. Install it from https://rustup.rs and open a new window.
goto :fail

:nonode
echo.
echo Node.js was not found. Install Node 22 from https://nodejs.org and open a new window.
goto :fail

:failpop
popd

:fail
echo.
echo The build failed. The messages above say why.
set "RC=1"

:end
if not defined NOPAUSE pause
exit /b %RC%
