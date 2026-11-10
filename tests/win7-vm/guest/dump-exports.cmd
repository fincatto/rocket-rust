@echo off
rem Rodar UMA VEZ numa VM Windows 7 SP1 x64 limpa (com as KBs do README).
rem Gera os JSONs de referencia usados pelo win7-check no CI.
rem Copie win7-check.exe (build x86_64-win7-windows-msvc) para o mesmo diretorio.
setlocal
set HERE=%~dp0
"%HERE%win7-check.exe" dump %SystemRoot%\System32 "%HERE%win7sp1-exports-x64.json" || goto :err
"%HERE%win7-check.exe" dump %SystemRoot%\SysWOW64 "%HERE%win7sp1-exports-x86.json" || goto :err
echo Pronto. Copie os dois JSON para tools\win7-check\data\ no repositorio.
goto :eof
:err
echo Falhou.
exit /b 1
