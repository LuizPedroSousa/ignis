#!/usr/bin/env bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_header() {
	echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
	echo -e "${BLUE}               Ignis Installer${NC}"
	echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

print_step() {
	echo -e "\n${GREEN}➜${NC} $1"
}

print_warning() {
	echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
	echo -e "${RED}✗${NC} $1"
}

print_success() {
	echo -e "${GREEN}✓${NC} $1"
}

check_rust() {
	print_step "Checking Rust installation..."
	if ! command -v cargo &>/dev/null; then
		print_error "Rust is not installed"
		echo ""
		echo "Please install Rust from https://rustup.rs:"
		echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
		exit 1
	fi

	local rust_version=$(rustc --version | awk '{print $2}')
	print_success "Found Rust $rust_version"

	local required_version="1.75"
	if ! cargo --version &>/dev/null; then
		print_error "Cargo is not available"
		exit 1
	fi
}

determine_install_location() {
	if [[ "$EUID" -eq 0 ]]; then
		INSTALL_DIR="/usr/local/bin"
		CONFIG_DIR="/etc/astralix"
	else
		INSTALL_DIR="$HOME/.local/bin"
		CONFIG_DIR="$HOME/.config/astralix"
	fi
}

build_ignis() {
	if [[ "$DEBUG" == "true" ]]; then
		print_step "Building ignis in debug mode..."
		BUILD_MODE="debug"
		BUILD_FLAGS=""
	else
		print_step "Building ignis in release mode..."
		BUILD_MODE="release"
		BUILD_FLAGS="--release"
	fi

	cd "$SCRIPT_DIR"

	if cargo build $BUILD_FLAGS --jobs "$(nproc)"; then
		print_success "Build completed successfully"
	else
		print_error "Build failed"
		exit 1
	fi

	BINARY_PATH="$SCRIPT_DIR/target/$BUILD_MODE/ignis"

	if [[ ! -f "$BINARY_PATH" ]]; then
		print_error "Binary not found at $BINARY_PATH"
		exit 1
	fi

	local binary_size=$(du -h "$BINARY_PATH" | awk '{print $1}')
	print_success "Binary size: $binary_size"
}

install_binary() {
	print_step "Installing binary to $INSTALL_DIR..."

	mkdir -p "$INSTALL_DIR"

	if cp "$BINARY_PATH" "$INSTALL_DIR/ignis"; then
		chmod +x "$INSTALL_DIR/ignis"
		print_success "Installed ignis to $INSTALL_DIR/ignis"
	else
		print_error "Failed to copy binary"
		exit 1
	fi
}

install_config() {
	print_step "Installing configuration..."

	mkdir -p "$CONFIG_DIR"

	if [[ -f "$SCRIPT_DIR/build.toml.example" ]]; then
		if [[ ! -f "$CONFIG_DIR/build.toml" ]]; then
			cp "$SCRIPT_DIR/build.toml.example" "$CONFIG_DIR/build.toml"
			print_success "Installed config to $CONFIG_DIR/build.toml"
		else
			print_warning "Config already exists at $CONFIG_DIR/build.toml (not overwriting)"
		fi
	fi
}

create_build_history_dir() {
	print_step "Creating build history directory..."

	local history_dir="$HOME/.astralix"
	mkdir -p "$history_dir"
	print_success "Created $history_dir"
}

verify_installation() {
	print_step "Verifying installation..."

	if command -v ignis &>/dev/null; then
		local installed_version=$(ignis --version 2>/dev/null || echo "unknown")
		print_success "ignis is available in PATH"
	else
		print_warning "ignis is not in PATH"
		echo ""
		echo "Add the following to your shell profile (~/.bashrc or ~/.zshrc):"
		echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
	fi
}

print_usage_instructions() {
	echo ""
	echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
	echo -e "${GREEN}Installation Complete!${NC}"
	echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
	echo ""
	echo "Usage:"
	echo "  ignis debug                # Build debug preset"
	echo "  ignis release              # Build release preset"
	echo "  ignis presets              # List available presets"
	echo "  ignis history              # Show build history"
	echo "  ignis --help               # Show all options"
	echo ""
	echo "Via wrapper (from project root):"
	echo "  ./buildtools/ignis.sh debug"
	echo ""
	echo "Configuration:"
	echo "  Config: $CONFIG_DIR/build.toml"
	echo "  History: $HOME/.astralix/build_history.json"
	echo ""
	echo "Documentation:"
	echo "  README: $SCRIPT_DIR/README.md"
	echo "  Quick Start: $SCRIPT_DIR/QUICK_START.md"
	echo ""
}

show_help() {
	echo "Usage: ./install.sh [OPTIONS]"
	echo ""
	echo "Options:"
	echo "  --help              Show this help message"
	echo "  --uninstall         Uninstall ignis"
	echo "  --debug             Build in debug mode instead of release"
	echo "  --prefix=DIR        Install to custom directory (default: ~/.local/bin or /usr/local/bin)"
	echo ""
	echo "Examples:"
	echo "  ./install.sh                    # Install to default location"
	echo "  ./install.sh --debug            # Build and install debug version"
	echo "  ./install.sh --prefix=/opt/bin  # Install to custom location"
	echo "  sudo ./install.sh               # Install system-wide to /usr/local/bin"
	echo ""
}

uninstall() {
	print_header
	print_step "Uninstalling ignis..."

	determine_install_location

	if [[ -f "$INSTALL_DIR/ignis" ]]; then
		rm -f "$INSTALL_DIR/ignis"
		print_success "Removed $INSTALL_DIR/ignis"
	else
		print_warning "Binary not found at $INSTALL_DIR/ignis"
	fi

	if [[ -f "$PROJECT_ROOT/buildtools/ignis.sh" ]]; then
		rm -f "$PROJECT_ROOT/buildtools/ignis.sh"
		print_success "Removed wrapper script"
	fi

	print_success "Uninstall complete"
	echo ""
	echo "Note: Configuration and build history were not removed:"
	echo "  Config: $CONFIG_DIR/"
	echo "  History: $HOME/.astralix/"
	echo ""
	echo "To remove these manually, run:"
	echo "  rm -rf $CONFIG_DIR"
	echo "  rm -rf $HOME/.astralix"
	echo ""
}

main() {
	DEBUG="false"

	if [[ "$1" == "--help" ]] || [[ "$1" == "-h" ]]; then
		show_help
		exit 0
	fi

	if [[ "$1" == "--uninstall" ]]; then
		uninstall
		exit 0
	fi

	print_header

	for arg in "$@"; do
		case $arg in
		--prefix=*)
			INSTALL_DIR="${arg#*=}"
			;;
		--debug)
			DEBUG="true"
			;;
		esac
	done

	check_rust
	determine_install_location
	build_ignis
	install_binary
	install_config
	create_build_history_dir
	verify_installation
	print_usage_instructions
}

main "$@"
