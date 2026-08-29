@echo off
echo ========================================
echo    Neeko Assistant - Inicio Rapido
echo ========================================
echo.

:: Verificar modelo GGUF
echo Verificando modelo...
if not exist "IA\neeko-qwen3-4b-Q4_K_M.gguf" (
    echo [ERROR] Modelo no encontrado en IA\neeko-qwen3-4b-Q4_K_M.gguf
    echo Descarga el modelo GGUF y ponelo en la carpeta IA\
    pause
    exit /b 1
)
echo Modelo OK.

:: Verificar Node
where node >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Node.js no esta instalado.
    pause
    exit /b 1
)

:: Verificar Tauri CLI
cargo tauri --version >nul 2>nul
if %errorlevel% neq 0 (
    echo Instalando Tauri CLI...
    cargo install tauri-cli --version "^2"
)

echo.
echo Iniciando Neeko Assistant...
call npx tauri dev

pause
