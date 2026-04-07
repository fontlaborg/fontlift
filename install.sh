#!/bin/bash
# install.sh - install the fontlift CLI on the current machine
# made by FontLab https://www.fontlab.com/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info()    { echo -e "${BLUE}[INFO]${NC} $1"; }
log_success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
log_warning() { echo -e "${YELLOW}[WARNING]${NC} $1"; }
log_error()   { echo -e "${RED}[ERROR]${NC} $1"; }

usage() {
    cat << EOF
Usage: $0 [OPTIONS]

Install the fontlift CLI binary on the current machine.

OPTIONS:
    --debug         Install debug build (faster compile, slower runtime)
    --path DIR      Install to DIR instead of ~/.cargo/bin
    -h, --help      Show this help

Examples:
    $0                        # Release install to ~/.cargo/bin
    $0 --path /usr/local/bin  # Install to custom location
    $0 --debug                # Debug build (faster compile)

EOF
}

INSTALL_DIR=""
BUILD_FLAGS="--release"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --debug)
            BUILD_FLAGS=""
            shift
            ;;
        --path)
            INSTALL_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            log_error "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Check for cargo
if ! command -v cargo >/dev/null 2>&1; then
    log_error "cargo not found. Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

if [[ -n "$INSTALL_DIR" ]]; then
    # Build and copy manually
    log_info "Building fontlift CLI..."
    cargo build -p fontlift-cli $BUILD_FLAGS --manifest-path "$SCRIPT_DIR/Cargo.toml"

    local_target="$SCRIPT_DIR/target"
    if [[ -n "$BUILD_FLAGS" ]]; then
        bin_path="$local_target/release/fontlift"
    else
        bin_path="$local_target/debug/fontlift"
    fi

    if [[ ! -f "$bin_path" ]]; then
        log_error "Build succeeded but binary not found at $bin_path"
        exit 1
    fi

    mkdir -p "$INSTALL_DIR"
    cp "$bin_path" "$INSTALL_DIR/fontlift"
    chmod +x "$INSTALL_DIR/fontlift"
    log_success "Installed fontlift to $INSTALL_DIR/fontlift"
else
    # Use cargo install (installs to ~/.cargo/bin by default)
    log_info "Installing fontlift CLI via cargo install..."
    cargo install --path "$SCRIPT_DIR/cli"
    CARGO_BIN="$HOME/.cargo/bin/fontlift"

    # If another fontlift binary shadows ~/.cargo/bin on PATH, update it too
    WHICH_BIN="$(which fontlift 2>/dev/null || true)"
    if [[ -n "$WHICH_BIN" && "$WHICH_BIN" != "$CARGO_BIN" && -f "$CARGO_BIN" ]]; then
        log_info "Updating shadowing binary at $WHICH_BIN..."
        cp "$CARGO_BIN" "$WHICH_BIN"
        chmod +x "$WHICH_BIN"
        log_success "Updated $WHICH_BIN"
    fi

    log_success "Installed fontlift to $(which fontlift 2>/dev/null || echo '~/.cargo/bin/fontlift')"
fi

# Verify
if command -v fontlift >/dev/null 2>&1; then
    log_success "fontlift is ready: $(fontlift --version 2>/dev/null || echo 'installed')"
else
    log_warning "fontlift was installed but is not on PATH. Add the install directory to your PATH."
fi
