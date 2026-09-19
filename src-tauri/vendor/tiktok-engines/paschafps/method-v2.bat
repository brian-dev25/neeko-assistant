@echo off
setlocal EnableExtensions EnableDelayedExpansion
chcp 65001 >nul

set "scriptDir=%~dp0"
set "ffmpegExe=%scriptDir%ffmpeg.exe"

if not exist "%ffmpegExe%" (
    cls
    echo ffmpeg.exe was not found in this folder.
    echo.
    echo Download ffmpeg yourself and place ffmpeg.exe next to this script.
    echo.
    pause
    exit /b 1
)

"%ffmpegExe%" -version >nul 2>&1
if errorlevel 1 (
    cls
    echo ffmpeg.exe was found but is not working correctly.
    echo.
    echo Replace it with a working ffmpeg.exe and try again.
    echo.
    pause
    exit /b 1
)

set "inputFile="
if not "%~1"=="" set "inputFile=%~1"
if not defined inputFile if exist "%scriptDir%input.mp4" set "inputFile=%scriptDir%input.mp4"

if not defined inputFile (
    for %%F in ("%scriptDir%*.mp4" "%scriptDir%*.mov" "%scriptDir%*.mkv" "%scriptDir%*.avi" "%scriptDir%*.webm" "%scriptDir%*.m4v") do (
        if not defined inputFile set "inputFile=%%~fF"
        if defined inputFile if /I not "%%~fF"=="!inputFile!" set "multipleFiles=1"
    )
)

if defined multipleFiles if /I not "%~1"=="" set "multipleFiles="

if defined multipleFiles (
    cls
    echo Multiple supported video files were found in this folder.
    echo.
    echo Drag and drop the exact file onto this script.
    echo.
    pause
    exit /b 1
)

if not defined inputFile (
    cls
    echo Drag and drop your video onto this .bat file
    echo or place input.mp4 next to it and run again.
    echo.
    pause
    exit /b 1
)

if not exist "%inputFile%" (
    cls
    echo input file not found.
    echo.
    pause
    exit /b 1
)

for %%I in ("%inputFile%") do (
    set "inputDir=%%~dpI"
    set "inputName=%%~nI"
    set "inputExt=%%~xI"
)

set "valid="
for %%E in (.mp4 .mov .mkv .avi .webm .m4v) do (
    if /I "!inputExt!"=="%%~E" set "valid=1"
)

if not defined valid (
    cls
    echo unsupported file format: %inputExt%
    echo.
    echo supported: .mp4 .mov .mkv .avi .webm .m4v
    echo.
    pause
    exit /b 1
)

if /I "%inputName:~-7%"=="_output" (
    set "baseName=%inputName:~0,-7%"
) else (
    set "baseName=%inputName%"
)

set "outputFile=%inputDir%%baseName%_output.mp4"

title paschafps Quality Method
:menu
cls
echo ____________________________________________________________
echo                    paschafps Quality Method
echo.
echo Enter your video's FPS (60, 120, 240):
set /p fps=^> 

set "fps=%fps: =%"

if "%fps%"=="60" set "scale=2"& goto process
if "%fps%"=="120" set "scale=6"& goto process
if "%fps%"=="240" set "scale=12"& goto process

echo.
echo invalid input.
timeout /t 2 >nul
goto menu

:process
if exist "%outputFile%" del /f /q "%outputFile%" >nul 2>&1

cls
echo processing...
echo.
"%ffmpegExe%" -y -itsscale %scale% -i "%inputFile%" -c:v copy -c:a copy "%outputFile%"

if errorlevel 1 (
    if exist "%outputFile%" del /f /q "%outputFile%" >nul 2>&1
    echo.
    echo failed.
    echo.
    pause
    exit /b 1
)

if not exist "%outputFile%" (
    echo.
    echo failed.
    echo.
    pause
    exit /b 1
)

echo.
echo done.
echo.
echo output: "%outputFile%"
echo.
pause
exit /b 0
