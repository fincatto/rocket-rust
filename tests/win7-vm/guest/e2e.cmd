@echo off
rem Executado DENTRO da VM Win7, na sessao interativa do usuario (via schtasks /it).
rem uso: e2e.cmd install <setup.exe> | selftest <tag> | launch | kill
setlocal
set OUT=C:\e2e\out
if not exist %OUT% mkdir %OUT%
set STEP=%1

if "%STEP%"=="install" (
  start "" /wait "%~2" /S
  call :finish %ERRORLEVEL%
  goto :eof
)

call :findapp
if not defined APP (
  echo rocket-rust.exe nao encontrado > %OUT%\%STEP%.log
  call :finish 3
  goto :eof
)

if "%STEP%"=="selftest" (
  "%APP%" --self-test %OUT%\selftest-%2.json
  call :finish %ERRORLEVEL%
  goto :eof
)
if "%STEP%"=="launch" (
  start "" "%APP%"
  call :finish 0
  goto :eof
)
if "%STEP%"=="kill" (
  taskkill /f /im rocket-rust.exe >nul 2>&1
  call :finish 0
  goto :eof
)
call :finish 2
goto :eof

:findapp
if exist "%LOCALAPPDATA%\Rocket Rust\rocket-rust.exe" set "APP=%LOCALAPPDATA%\Rocket Rust\rocket-rust.exe"
if defined APP goto :eof
for /f "delims=" %%f in ('where /r "%LOCALAPPDATA%" rocket-rust.exe 2^>nul') do set "APP=%%f"
goto :eof

:finish
echo %1> %OUT%\done-%STEP%.txt
goto :eof
