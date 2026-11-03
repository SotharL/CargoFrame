@echo off
rem Try the one-click Claude delivery by hand and keep a log of what happened.
rem
rem   scripts\windows\try-claude.cmd                 uses the chat link in scripts\windows\try-claude.link.txt
rem   scripts\windows\try-claude.cmd <link> [--browser] [--no-enter] [--extra N] [--no-file]
rem
rem (The variable is not called LINK: the linker reads that one and treats it as a file to link.)
rem The log is try-claude.log in the repository folder. Neither the log nor the link file is
rem committed. It posts a clearly marked test message, so point it at a chat made for testing.
setlocal
cd /d "%~dp0\..\.."
set "LOG=%CD%\try-claude.log"
if "%~1"=="" (
  if not exist "%~dp0try-claude.link.txt" (
    echo Put a Claude chat link in scripts\windows\try-claude.link.txt, or pass it as the first argument.
    pause
    exit /b 2
  )
  set /p CHATLINK=<"%~dp0try-claude.link.txt"
  set "TRYARGS=%CHATLINK%"
) else (
  set "TRYARGS=%*"
)
echo Delivery test started %DATE% %TIME% > "%LOG%"
cargo run -q -p cargoframe-deliver --example try_claude -- %TRYARGS% >> "%LOG%" 2>&1
echo Exit code %ERRORLEVEL% >> "%LOG%"
type "%LOG%"
echo.
echo Finished. This window can be closed.
pause
