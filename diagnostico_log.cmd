@echo off
rem ---------------------------------------------------------------------------
rem  Lanza LGBCADStudio con la salida capturada en un archivo.
rem  Doble clic, reproduce el fallo y cierra el programa como lo cerraste.
rem  Al final la ventana queda abierta: pega el contenido de salida.log
rem  (o dime y lo leo yo del workspace).
rem ---------------------------------------------------------------------------
setlocal
cd /d "%~dp0"
set RUST_BACKTRACE=full
set RUST_LOG=info
set SALIDA=%~dp0salida.log

echo === LGBCADStudio con log === > "%SALIDA%"
echo Inicio: %DATE% %TIME% >> "%SALIDA%"
echo Revision: >> "%SALIDA%"
git -C "%~dp0" rev-parse --short HEAD >> "%SALIDA%" 2>&1
echo. >> "%SALIDA%"

"%~dp0target\debug\lgbcadstudio.exe" --log debug >> "%SALIDA%" 2>&1
set CODE=%ERRORLEVEL%

echo. >> "%SALIDA%"
echo === FIN : codigo de salida %CODE% : %DATE% %TIME% === >> "%SALIDA%"

echo.
echo ==========================================================
echo   Codigo de salida: %CODE%
echo   Log: %SALIDA%
echo ==========================================================
echo.
echo   (0 = cierre normal, 101 = panic de Rust, otro = crash nativo)
echo.
pause
