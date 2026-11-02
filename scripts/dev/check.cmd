@echo off
setlocal EnableExtensions
rem ----------------------------------------------------------------------------------------
rem  The local gate on Windows: what CI checks, run on this computer first.
rem
rem    scripts\dev\check.cmd           format, lint, every Rust test, type check, UI tests, UI build
rem    scripts\dev\check.cmd quick     format, type check and UI tests only
rem    scripts\dev\check.cmd nopause   close without waiting for a key
rem
rem  Push only when this passes: CI minutes are limited. Variables start with CF_ on purpose
rem  (see build.cmd). Tools are started as .cmd files so PowerShell's script setting cannot block them.
rem ----------------------------------------------------------------------------------------
set "CF_QUICK="
set "CF_NOPAUSE="
set "CF_FAILED="
for %%A in (%*) do call :option "%%~A"
cd /d "%~dp0..\.."

echo --- cargo fmt --check
cargo fmt --all --check
if errorlevel 1 set "CF_FAILED=%CF_FAILED% fmt"

if defined CF_QUICK goto :ui
echo --- cargo clippy
cargo clippy --workspace --all-targets -- -D warnings
if errorlevel 1 set "CF_FAILED=%CF_FAILED% clippy"
echo --- cargo test
cargo test --workspace --no-fail-fast
if errorlevel 1 set "CF_FAILED=%CF_FAILED% rust-tests"

:ui
pushd ui
echo --- svelte-check
call npx.cmd svelte-check
if errorlevel 1 set "CF_FAILED=%CF_FAILED% svelte-check"
echo --- vitest
call npx.cmd vitest run
if errorlevel 1 set "CF_FAILED=%CF_FAILED% vitest"
if defined CF_QUICK goto :uidone
echo --- ui build
call npm.cmd run build
if errorlevel 1 set "CF_FAILED=%CF_FAILED% ui-build"
:uidone
popd

if defined CF_QUICK goto :summary
echo --- size budgets
node scripts\ci\check-budgets.mjs
if errorlevel 1 set "CF_FAILED=%CF_FAILED% size-budgets"

:summary
echo.
if defined CF_FAILED goto :bad
echo All checks passed. Safe to push.
set "CF_RC=0"
goto :end

:bad
echo Failed:%CF_FAILED%
set "CF_RC=1"
goto :end

:option
if /i "%~1"=="quick"   set "CF_QUICK=1"
if /i "%~1"=="nopause" set "CF_NOPAUSE=1"
exit /b 0

:end
if not defined CF_NOPAUSE pause
exit /b %CF_RC%
