@echo off
echo ========================================
echo    Neeko Assistant - Build Produccion
echo ========================================
echo.
echo Construyendo .exe (esto tarda unos minutos)...
call npx tauri build
echo.
echo Listo! El .exe esta en:
echo src-tauri/target/release/bundle/nsis/
echo.
pause
