@echo off
setlocal EnableExtensions

rem BDE Windows client build from a complete PLANK checkout.
rem Requires MSVC 2022 Build Tools, Rust 1.99.1, Qt 6.10.2 msvc2022_64,
rem and the pinned Windows dependency archive under apps\client\libs\windows.
rem The native PLANK transport is mandatory. Set PLANK_WINDOWS_RELEASE_BUILD=1
rem for the unmodified release version; packaging and signing are separate.

for %%I in ("%~dp0..\..") do set "PLANK_SOURCE_ROOT=%%~fI"
if not defined PLANK_QT_ROOT (
    echo PLANK_QT_ROOT must name the Qt 6.10.2 msvc2022_64 directory. 1>&2
    exit /b 2
)
if not defined PLANK_WINDOWS_BUILD_DIR (
    echo PLANK_WINDOWS_BUILD_DIR must name an isolated output directory. 1>&2
    exit /b 2
)
if not exist "%PLANK_QT_ROOT%\bin\qmake.exe" (
    echo Qt qmake.exe is missing from PLANK_QT_ROOT. 1>&2
    exit /b 2
)
if not exist "%PLANK_SOURCE_ROOT%\apps\client\libs\windows\include" (
    echo Windows client dependencies are missing. 1>&2
    exit /b 2
)
if not exist "%PLANK_SOURCE_ROOT%\packaging\VERSION" (
    echo PLANK release version is missing. 1>&2
    exit /b 2
)
for /f "usebackq delims=" %%I in ("%PLANK_SOURCE_ROOT%\packaging\VERSION") do set "PLANK_RELEASE_VERSION=%%I"
set "PLANK_CLIENT_VERSION=%PLANK_RELEASE_VERSION%-windows-dev"
if defined PLANK_BUILD_BRANCH if /I not "%PLANK_BUILD_BRANCH%"=="main" set "PLANK_CLIENT_VERSION=%PLANK_RELEASE_VERSION%-%PLANK_BUILD_BRANCH%"
if "%PLANK_WINDOWS_RELEASE_BUILD%"=="1" set "PLANK_CLIENT_VERSION=%PLANK_RELEASE_VERSION%"

set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
if not exist "%VSWHERE%" (
    echo Visual Studio vswhere.exe is missing. 1>&2
    exit /b 2
)
set "VS_ROOT="
for /f "usebackq delims=" %%I in (`"%VSWHERE%" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VS_ROOT=%%I"
if not defined VS_ROOT (
    echo The MSVC x64 tools are missing. 1>&2
    exit /b 2
)
call "%VS_ROOT%\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 exit /b 1

set "PATH=%PLANK_QT_ROOT%\bin;%PATH%"
set "CARGO_TARGET_DIR=%USERPROFILE%\.cargo\shared-target"
if not exist "%PLANK_WINDOWS_BUILD_DIR%\qmake" mkdir "%PLANK_WINDOWS_BUILD_DIR%\qmake"
if errorlevel 1 exit /b 1

cargo +1.99.1 build --locked --release --target x86_64-pc-windows-msvc --manifest-path "%PLANK_SOURCE_ROOT%\protocol\plank-transport\Cargo.toml"
if errorlevel 1 exit /b 1
set "PLANK_TRANSPORT_LIBRARY=%CARGO_TARGET_DIR%\x86_64-pc-windows-msvc\release\plank_transport.lib"
if not exist "%PLANK_TRANSPORT_LIBRARY%" (
    echo The native PLANK transport library was not produced. 1>&2
    exit /b 1
)

pushd "%PLANK_WINDOWS_BUILD_DIR%\qmake"
"%PLANK_QT_ROOT%\bin\qmake.exe" -r "%PLANK_SOURCE_ROOT%\apps\client\moonlight-qt.pro" CONFIG+=release CONFIG+=plank-transport "PLANK_VERSION=%PLANK_CLIENT_VERSION%" "PLANK_TRANSPORT_DIR=%PLANK_SOURCE_ROOT%\protocol\plank-transport" "PLANK_TRANSPORT_LIBRARY=%PLANK_TRANSPORT_LIBRARY%"
if errorlevel 1 goto :failure
nmake /NOLOGO /S release
if errorlevel 1 goto :failure
popd
echo BDE fernweh Windows client %PLANK_CLIENT_VERSION% build completed.
exit /b 0

:failure
set "BUILD_ERROR=%ERRORLEVEL%"
popd
exit /b %BUILD_ERROR%
