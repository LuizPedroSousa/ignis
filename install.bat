@echo off
setlocal enabledelayedexpansion

set SCRIPT_DIR=%~dp0
set SCRIPT_DIR=%SCRIPT_DIR:~0,-1%
for %%i in ("%SCRIPT_DIR%\..") do set PROJECT_ROOT=%%~fi

set RED=[31m
set GREEN=[32m
set YELLOW=[33m
set BLUE=[34m
set NC=[0m

if "%1"=="--help" goto :show_help
if "%1"=="-h" goto :show_help
if "%1"=="--uninstall" goto :uninstall

call :print_header
call :check_rust
call :determine_install_location
call :build_ignis
call :install_binary
call :install_config
call :create_build_history_dir
call :verify_installation
call :print_usage_instructions
goto :eof

:print_header
echo %BLUE%━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%NC%
echo %BLUE%               Ignis Installer%NC%
echo %BLUE%━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%NC%
goto :eof

:print_step
echo.
echo %GREEN%^>%NC% %~1
goto :eof

:print_warning
echo %YELLOW%!%NC% %~1
goto :eof

:print_error
echo %RED%X%NC% %~1
goto :eof

:print_success
echo %GREEN%+%NC% %~1
goto :eof

:check_rust
call :print_step "Checking Rust installation..."
where cargo >nul 2>&1
if %errorlevel% neq 0 (
    call :print_error "Rust is not installed"
    echo.
    echo Please install Rust from https://rustup.rs:
    echo   Download and run: https://win.rustup.rs/x86_64
    echo   Or use winget: winget install Rustlang.Rustup
    exit /b 1
)

for /f "tokens=2" %%i in ('rustc --version') do (
    call :print_success "Found Rust %%i"
    goto :check_rust_done
)
:check_rust_done
goto :eof

:determine_install_location
net session >nul 2>&1
if %errorlevel% equ 0 (
    set INSTALL_DIR=%ProgramFiles%\Astralix\bin
    set CONFIG_DIR=%ProgramData%\astralix
) else (
    set INSTALL_DIR=%USERPROFILE%\.local\bin
    set CONFIG_DIR=%APPDATA%\astralix
)
goto :eof

:build_ignis
call :print_step "Building ignis in release mode..."
cd /d "%SCRIPT_DIR%"

set NUM_PROCESSORS=%NUMBER_OF_PROCESSORS%
cargo build --release --jobs %NUM_PROCESSORS%
if %errorlevel% neq 0 (
    call :print_error "Build failed"
    exit /b 1
)

call :print_success "Build completed successfully"

set BINARY_PATH=%SCRIPT_DIR%\target\release\ignis.exe

if not exist "%BINARY_PATH%" (
    call :print_error "Binary not found at %BINARY_PATH%"
    exit /b 1
)

for %%A in ("%BINARY_PATH%") do set BINARY_SIZE=%%~zA
set /a BINARY_SIZE_MB=!BINARY_SIZE! / 1048576
call :print_success "Binary size: !BINARY_SIZE_MB! MB"
goto :eof

:install_binary
call :print_step "Installing binary to %INSTALL_DIR%..."

if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

copy /Y "%BINARY_PATH%" "%INSTALL_DIR%\ignis.exe" >nul
if %errorlevel% neq 0 (
    call :print_error "Failed to copy binary"
    exit /b 1
)

call :print_success "Installed ignis to %INSTALL_DIR%\ignis.exe"
goto :eof

:install_config
call :print_step "Installing configuration..."

if not exist "%CONFIG_DIR%" mkdir "%CONFIG_DIR%"

if exist "%SCRIPT_DIR%\ignis.toml.example" (
    if not exist "%CONFIG_DIR%\ignis.toml" (
        copy "%SCRIPT_DIR%\ignis.toml.example" "%CONFIG_DIR%\ignis.toml" >nul
        call :print_success "Installed config to %CONFIG_DIR%\ignis.toml"
    ) else (
        call :print_warning "Config already exists at %CONFIG_DIR%\ignis.toml (not overwriting)"
    )
)
goto :eof

:create_build_history_dir
call :print_step "Creating build history directory..."

set HISTORY_DIR=%USERPROFILE%\.astralix
if not exist "%HISTORY_DIR%" mkdir "%HISTORY_DIR%"
call :print_success "Created %HISTORY_DIR%"
goto :eof

:verify_installation
call :print_step "Verifying installation..."

where ignis >nul 2>&1
if %errorlevel% equ 0 (
    call :print_success "ignis is available in PATH"
) else (
    call :print_warning "ignis is not in PATH"
    echo.
    echo Add the following to your PATH:
    echo   1. Press Win+X, select 'System'
    echo   2. Click 'Advanced system settings'
    echo   3. Click 'Environment Variables'
    echo   4. Edit 'Path' and add: %INSTALL_DIR%
    echo.
    echo Or run this command in an Administrator PowerShell:
    echo   [Environment]::SetEnvironmentVariable('Path', $env:Path + ';%INSTALL_DIR%', 'User'^)
)
goto :eof

:print_usage_instructions
echo.
echo %BLUE%━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%NC%
echo %GREEN%Installation Complete!%NC%
echo %BLUE%━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━%NC%
echo.
echo Usage:
echo   ignis debug                # Build debug preset
echo   ignis release              # Build release preset
echo   ignis presets              # List available presets
echo   ignis history              # Show build history
echo   ignis --help               # Show all options
echo.
echo Configuration:
echo   Config: %CONFIG_DIR%\ignis.toml
echo   History: %USERPROFILE%\.astralix\build_history.json
echo.
goto :eof

:show_help
echo Usage: install.bat [OPTIONS]
echo.
echo Options:
echo   --help              Show this help message
echo   --uninstall         Uninstall ignis
echo.
echo Examples:
echo   install.bat                    # Install to default location
echo   install.bat --uninstall        # Uninstall ignis
echo.
echo Run as Administrator to install system-wide to Program Files
echo.
goto :eof

:uninstall
call :print_header
call :print_step "Uninstalling ignis..."
call :determine_install_location

if exist "%INSTALL_DIR%\ignis.exe" (
    del /F "%INSTALL_DIR%\ignis.exe"
    call :print_success "Removed %INSTALL_DIR%\ignis.exe"
) else (
    call :print_warning "Binary not found at %INSTALL_DIR%\ignis.exe"
)

call :print_success "Uninstall complete"
echo.
echo Note: Configuration and build history were not removed:
echo   Config: %CONFIG_DIR%
echo   History: %USERPROFILE%\.astralix
echo.
echo To remove these manually, run:
echo   rmdir /S /Q "%CONFIG_DIR%"
echo   rmdir /S /Q "%USERPROFILE%\.astralix"
echo.
goto :eof
