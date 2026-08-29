@echo off
echo ========================================
echo    Verificar modelo Neeko
echo ========================================
echo.
echo Verificando que el modelo GGUF exista...
echo.

if exist "IA\neeko-qwen3-4b-Q4_K_M.gguf" (
    echo [OK] Modelo encontrado: IA\neeko-qwen3-4b-Q4_K_M.gguf
    echo.
    echo El modelo ya está listo. No necesitás Ollama.
    echo llama-server lo carga automáticamente al iniciar la app.
) else (
    echo [ERROR] Modelo no encontrado.
    echo.
    echo Descargá el modelo GGUF y ponelo en la carpeta IA\
    echo Nombre esperado: neeko-qwen3-4b-Q4_K_M.gguf
)

echo.
pause
