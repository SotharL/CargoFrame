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
rem  Every variable here starts with CF_ because the build tools read some plain names from the
rem  environment (a variable called RC makes the Windows resource compiler run a program of that name).
rem
rem  This is a cmd script on purpose. In PowerShell, "npm" runs npm.ps1, which Windows blocks
rem  when script running is disabled. Here every tool is started as its .cmd file instead.
rem ----------------------------------------------------------------------------------------
set "CF_MODE=installer"
set "CF_FAST="
set "CF_NOPAUSE="
set "CF_RC=0"
set "CF_BADOPTION="

for %%A in (%*) do call :option "%%~A"
if defined CF_BADOPTION goto :usage

cd /d "%~dp0..\.."
echo CargoFrame build in %CD%
echo.

where cargo >nul 2>&1
if errorlevel 1 goto :nocargo
where node >nul 2>&1
if errorlevel 1 goto :nonode

if defined CF_FAST if exist "ui\node_modules" goto :build
echo [1/2] Installing the web packages ...
pushd ui
call npm.cmd ci --no-audit --no-fund
if errorlevel 1 goto :failpop
popd

:build
if /i "%CF_MODE%"=="program" goto :buildprogram
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
if /i "%CF_MODE%"=="program" goto :end

set "CF_SETUP="
for /f "delims=" %%F in ('dir /b /o-d "target\release\bundle\nsis\*-setup.exe" 2^>nul') do if not defined CF_SETUP set "CF_SETUP=%CD%\target\release\bundle\nsis\%%F"
if not defined CF_SETUP goto :fail
echo   Installer: %CF_SETUP%
if /i "%CF_MODE%"=="install" start "" "%CF_SETUP%"
goto :end

:option
if /i "%~1"=="program"   ( set "CF_MODE=program"   & exit /b 0 )
if /i "%~1"=="installer" ( set "CF_MODE=installer" & exit /b 0 )
if /i "%~1"=="install"   ( set "CF_MODE=install"   & exit /b 0 )
if /i "%~1"=="fast"      ( set "CF_FAST=1"         & exit /b 0 )
if /i "%~1"=="nopause"   ( set "CF_NOPAUSE=1"      & exit /b 0 )
echo Unknown option: %~1
set "CF_BADOPTION=1"
exit /b 0

:usage
echo.
echo Usage: build.cmd [program ^| installer ^| install] [fast] [nopause]
set "CF_RC=2"
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
set "CF_RC=1"

:end
if not defined CF_NOPAUSE pause
exit /b %CF_RC%
