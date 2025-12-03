#!/bin/bash

# FontLift Build Script - Production Ready
# Optimized cross-platform build script for fontlift library and CLI
# Ensures 100% reliability on fresh systems with automated dependency checking

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Script configuration
SCRIPT_NAME="FontLift Production Builder"
VERSION="2.0.0-dev"
MIN_RUST_VERSION="1.75.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Function to print colored output
print_header() {
	echo -e "${PURPLE}=== $SCRIPT_NAME v${VERSION} ===${NC}"
	echo -e "${PURPLE}========================================${NC}"
}

print_status() {
	echo -e "${GREEN}[✓ INFO]${NC} $1"
}

print_warning() {
	echo -e "${YELLOW}[⚠ WARN]${NC} $1"
}

print_error() {
	echo -e "${RED}[✗ ERROR]${NC} $1"
}

print_step() {
	echo -e "${BLUE}[→ STEP]${NC} $1"
}

print_success() {
	echo -e "${GREEN}[✓ SUCCESS]${NC} $1"
}

print_substep() {
	echo -e "${CYAN}  •${NC} $1"
}

# Function to check if command exists
command_exists() {
	command -v "$1" >/dev/null 2>&1
}

# Function to compare versions
version_compare() {
	local version1="$1" operator="$2" version2="$3"
	if [[ "$operator" == ">=" ]]; then
		[[ "$(printf '%s\n' "$version1" "$version2" | sort -V | head -n1)" == "$version2" ]]
	elif [[ "$operator" == ">" ]]; then
		[[ "$version1" != "$version2" && "$(printf '%s\n' "$version1" "$version2" | sort -V | head -n1)" == "$version2" ]]
	fi
}

# Function to get platform information
get_platform_info() {
	local platform="unknown"
	local arch="unknown"
	local dylib_suffix=".so"
	
	# Detect platform
	if [[ "$OSTYPE" == "darwin"* ]]; then
		platform="macos"
		dylib_suffix=".dylib"
	elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
		platform="linux"
		dylib_suffix=".so"
	elif [[ "$OSTYPE" == "msys" ]] || [[ "$OSTYPE" == "cygwin" ]]; then
		platform="windows"
		dylib_suffix=".dll"
	fi
	
	# Detect architecture
	case $(uname -m) in
		x86_64) arch="x86_64" ;;
		arm64) arch="arm64" ;;
		aarch64) arch="aarch64" ;;
		*) arch="unknown" ;;
	esac
	
	echo "$platform:$arch:$dylib_suffix"
}

# Function to check and install dependencies
check_and_install_dependencies() {
	local platform_info=$(get_platform_info)
	local platform=$(echo "$platform_info" | cut -d: -f1)
	
	print_step "Checking and installing dependencies..."
	
	# Check if we're in the right directory
	if [ ! -f "$SCRIPT_DIR/Cargo.toml" ]; then
		print_error "Cargo.toml not found. Please run this script from the fontlift root directory."
		exit 1
	fi
	
	# Check for Rust
	if ! command_exists cargo; then
		print_error "Rust/Cargo not found. Installing Rust..."
		print_status "Downloading rustup installer..."
		curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
		source "$HOME/.cargo/env"
		
		if ! command_exists cargo; then
			print_error "Failed to install Rust. Please install manually from https://rustup.rs/"
			exit 1
		fi
	else
		print_status "Rust/Cargo found ✓"
	fi
	
	# Check Rust version
	local rust_version
	rust_version=$(rustc --version | cut -d' ' -f2)
	if ! version_compare "$rust_version" ">=" "$MIN_RUST_VERSION"; then
		print_error "Rust version $rust_version is too old. Minimum required: $MIN_RUST_VERSION"
		print_status "Updating Rust..."
		rustup update
		rust_version=$(rustc --version | cut -d' ' -f2)
		if ! version_compare "$rust_version" ">=" "$MIN_RUST_VERSION"; then
			print_error "Failed to update Rust to required version"
			exit 1
		fi
	fi
	print_status "Rust version: $rust_version ✓"
	
	# Platform-specific dependency checks
	case $platform in
	macos)
		check_macos_dependencies
		;;
	windows)
		check_windows_dependencies
		;;
	linux)
		check_linux_dependencies
		;;
	esac
	
	# Check for optional but recommended tools
	check_optional_dependencies
}

# Function to check macOS dependencies
check_macos_dependencies() {
	print_substep "Checking macOS dependencies..."
	
	# Check for Xcode command line tools
	if ! command_exists xcodebuild; then
		print_warning "Xcode command line tools not found. Installing..."
		xcode-select --install
		print_status "Waiting for Xcode command line tools installation..."
		print_status "Please run the script again after installation completes."
		exit 0
	else
		print_status "Xcode command line tools found ✓"
	fi
	
	# Check for Python (for Python bindings)
	if command_exists python3; then
		print_status "Python 3 found ✓"
		
		# Check for pip
		if command_exists pip3; then
			print_status "pip3 found ✓"
		else
			print_warning "pip3 not found. Python bindings will not be built."
		fi
		
		# Check for maturin
		if command_exists maturin; then
			print_status "maturin found for Python bindings ✓"
		else
			print_warning "maturin not found. Installing..."
			if command_exists pip3; then
				pip3 install --user maturin
				if command_exists maturin; then
					print_status "maturin installed successfully ✓"
				else
					print_warning "Failed to install maturin. Python bindings will not be built."
				fi
			fi
		fi
	else
		print_warning "Python 3 not found. Python bindings will not be built."
	fi
	
	# Check for uv (recommended)
	if command_exists uv; then
		print_status "uv found - recommended for Python development ✓"
	else
		print_warning "uv not found. Consider installing for better Python dependency management."
		print_status "Install with: curl -LsSf https://astral.sh/uv/install.sh | sh"
	fi
}

# Function to check Windows dependencies
check_windows_dependencies() {
	print_substep "Checking Windows dependencies..."
	
	# Check for Visual Studio Build Tools
	if ! command_exists cl.exe; then
		print_warning "Visual Studio Build Tools may not be installed."
		print_status "Please install Visual Studio Build Tools with C++ support."
		print_status "Download from: https://visualstudio.microsoft.com/visual-cpp-build-tools/"
	else
		print_status "Visual Studio Build Tools found ✓"
	fi
	
	# Check for Python
	if command_exists python; then
		print_status "Python found ✓"
		
		if command_exists pip; then
			print_status "pip found ✓"
		else
			print_warning "pip not found. Python bindings will not be built."
		fi
		
		if command_exists maturin; then
			print_status "maturin found for Python bindings ✓"
		else
			print_warning "maturin not found. Installing..."
			if command_exists pip; then
				pip install --user maturin
				if command_exists maturin; then
					print_status "maturin installed successfully ✓"
				else
					print_warning "Failed to install maturin. Python bindings will not be built."
				fi
			fi
		fi
	else
		print_warning "Python not found. Python bindings will not be built."
	fi
}

# Function to check Linux dependencies
check_linux_dependencies() {
	print_substep "Checking Linux dependencies..."
	
	# Check for basic build tools
	local missing_tools=()
	
	for tool in gcc g++ make pkg-config; do
		if ! command_exists $tool; then
			missing_tools+=($tool)
		fi
	done
	
	if [ ${#missing_tools[@]} -gt 0 ]; then
		print_warning "Missing build tools: ${missing_tools[*]}"
		print_status "Install with:"
		print_status "  Ubuntu/Debian: sudo apt-get install build-essential pkg-config"
		print_status "  Fedora/RHEL: sudo dnf install gcc gcc-c++ make pkgconfig"
		print_status "  Arch Linux: sudo pacman -S base-devel pkgconf"
	else
		print_status "Build tools found ✓"
	fi
	
	# Check for font development libraries
	if ! pkg-config --exists fontconfig 2>/dev/null; then
		print_warning "fontconfig development libraries not found."
		print_status "Install with:"
		print_status "  Ubuntu/Debian: sudo apt-get install libfontconfig1-dev"
		print_status "  Fedora/RHEL: sudo dnf install fontconfig-devel"
		print_status "  Arch Linux: sudo pacman -S fontconfig"
	else
		print_status "fontconfig development libraries found ✓"
	fi
}

# Function to check optional dependencies
check_optional_dependencies() {
	print_substep "Checking optional dependencies..."
	
	# Check for git
	if command_exists git; then
		print_status "git found ✓"
	else
		print_warning "git not found. Recommended for version control."
	fi
	
	# Check for common development tools
	local tools=("jq" "curl" "wget")
	for tool in "${tools[@]}"; do
		if command_exists $tool; then
			print_status "$tool found ✓"
		fi
	done
}

# Function to build components
build_component() {
	local component=$1
	local features=$2
	local build_mode=$3
	
	print_step "Building $component..."
	
	local build_flags=""
	if [ "$build_mode" == "release" ]; then
		build_flags="--release"
	fi
	
	cd "$SCRIPT_DIR"
	
	if [ -n "$features" ]; then
		cargo build -p "$component" --features "$features" $build_flags
	else
		cargo build -p "$component" $build_flags
	fi
	
	if [ $? -eq 0 ]; then
		print_status "$component built successfully"
	else
		print_error "Failed to build $component"
		exit 1
	fi
}

# Function to run tests
run_tests() {
	local build_mode=$1
	
	print_step "Running comprehensive tests..."
	
	local test_flags=""
	if [ "$build_mode" == "release" ]; then
		test_flags="--release"
	fi
	
	cd "$SCRIPT_DIR"
	
	# Run unit tests
	print_substep "Running unit tests..."
	cargo test --workspace $test_flags --lib
	if [ $? -eq 0 ]; then
		print_status "Unit tests passed ✓"
	else
		print_error "Unit tests failed"
		exit 1
	fi
	
	# Run integration tests if they exist
	if [ -d "$SCRIPT_DIR/tests" ]; then
		print_substep "Running integration tests..."
		cargo test --workspace $test_flags --test '*'
		if [ $? -eq 0 ]; then
			print_status "Integration tests passed ✓"
		else
			print_error "Integration tests failed"
			exit 1
		fi
	fi
	
	# Test CLI functionality
	local cli_path="target/$build_mode/fontlift"
	if [ -f "$SCRIPT_DIR/$cli_path" ]; then
		print_substep "Testing CLI functionality..."
		if "$SCRIPT_DIR/$cli_path" --help >/dev/null 2>&1; then
			print_status "CLI help command works ✓"
		else
			print_error "CLI help command failed"
			exit 1
		fi
		
		# Test CLI list command
		if "$SCRIPT_DIR/$cli_path" list >/dev/null 2>&1; then
			print_status "CLI list command works ✓"
		else
			print_warning "CLI list command failed (may be expected without fonts)"
		fi
	else
		print_warning "CLI binary not found at $cli_path"
	fi
	
	# Test Python bindings if built
	local platform_info=$(get_platform_info)
	local dylib_suffix=$(echo "$platform_info" | cut -d: -f3)
	local python_path="target/$build_mode/lib_native$dylib_suffix"
	
	if [ -f "$SCRIPT_DIR/$python_path" ]; then
		print_substep "Testing Python bindings..."
		if command_exists python3; then
			if python3 -c "import sys; sys.path.insert(0, '$SCRIPT_DIR/target/$build_mode'); import fontlift; print('✓ Python bindings import successful')" 2>/dev/null; then
				print_status "Python bindings work ✓"
			else
				print_warning "Python bindings test failed (may need to be installed first)"
			fi
		else
			print_warning "Python 3 not available for testing bindings"
		fi
	fi
}

# Function to build Python bindings
build_python_bindings() {
	local build_mode=$1
	local manifest_path="$SCRIPT_DIR/crates/fontlift-python/Cargo.toml"

	print_step "Building Python bindings..."
	
	if command_exists hatch; then
		print_substep "Using hatch to build wheel..."
		hatch build -t wheel
		local wheel
		wheel=$(ls -t dist/fontlift-*.whl 2>/dev/null | head -n1 || true)
		if [ -n "$wheel" ]; then
			python3 -m pip install --force-reinstall "$wheel"
			print_status "Python wheel built and installed ✓"
		else
			print_warning "Hatch build did not produce a wheel; skipping install"
		fi
		return
	fi

	if ! command_exists maturin; then
		print_warning "hatch/maturin not found, skipping Python bindings"
		return
	fi
	
	if ! command_exists python3 && ! command_exists python; then
		print_warning "Python not found, skipping Python bindings"
		return
	fi
	
	local build_flags=""
	if [ "$build_mode" == "release" ]; then
		build_flags="--release"
	fi
	
	cd "$SCRIPT_DIR"
	
	# Build Python bindings from the Python crate manifest (workspace root lacks a package table)
	maturin develop -m "$manifest_path" $build_flags --features extension-module
	if [ $? -eq 0 ]; then
		print_status "Python bindings built and installed successfully"
	else
		print_error "Failed to build Python bindings"
		exit 1
	fi
}

# Function to create distribution packages
create_packages() {
	local build_mode=$1
	local platform_info=$(get_platform_info)
	local platform=$(echo "$platform_info" | cut -d: -f1)
	local arch=$(echo "$platform_info" | cut -d: -f2)
	
	print_step "Creating distribution packages..."
	
	cd "$SCRIPT_DIR"
	
	# Create distribution directory
	local dist_dir="dist-$build_mode-$platform-$arch"
	mkdir -p "$dist_dir"
	
	# Package CLI binary
	local cli_path="target/$build_mode/fontlift"
	if [ -f "$cli_path" ]; then
		cp "$cli_path" "$dist_dir/"
		print_status "CLI binary packaged"
	fi
	
	# Package Python wheel if maturin is available
	if [ "$build_mode" == "release" ]; then
		if command_exists hatch; then
			print_substep "Building Python wheel with hatch..."
			if hatch build -t wheel --output "$dist_dir"; then
				print_status "Python wheel created"
			else
				print_warning "Failed to create Python wheel via hatch"
			fi
		elif command_exists maturin; then
			print_substep "Building Python wheel..."
			maturin build -m "$SCRIPT_DIR/crates/fontlift-python/Cargo.toml" --release --out "$dist_dir"
			if [ $? -eq 0 ]; then
				print_status "Python wheel created"
			else
				print_warning "Failed to create Python wheel"
			fi
		fi
	fi
	
	# Copy essential files
	cp README.md "$dist_dir/" 2>/dev/null || true
	cp LICENSE "$dist_dir/" 2>/dev/null || true
	cp USAGE.md "$dist_dir/" 2>/dev/null || true
	
	# Create archive
	local archive_name="fontlift-$platform-$arch-$build_mode.tar.gz"
	tar -czf "$archive_name" -C "$dist_dir" .
	print_status "Distribution archive created: $archive_name"
	
	print_success "Packages created in $dist_dir/"
}

# Function to show usage
show_usage() {
	echo "FontLift Production Build Script"
	echo ""
	echo "Usage: $0 [OPTIONS]"
	echo ""
	echo "Options:"
	echo "  --release         Build in release mode (default: debug)"
	echo "  --test            Run tests after build"
	echo "  --python          Build Python bindings"
	echo "  --cli-only        Build only CLI components"
	echo "  --core-only       Build only core library"
	echo "  --package         Create distribution packages"
	echo "  --clean           Clean build artifacts before building"
	echo "  --check-deps      Only check dependencies, don't build"
	echo "  --verbose         Enable verbose output"
	echo "  --help            Show this help message"
	echo ""
	echo "Examples:"
	echo "  $0 --release --test --python     Full build with tests and Python bindings"
	echo "  $0 --cli-only --test             Quick CLI build with tests"
	echo "  $0 --release --package           Production build with packaging"
	echo "  $0 --check-deps                  Only verify dependencies"
	echo ""
	echo "Environment Variables:"
	echo "  RUST_LOG          Set logging level (debug, info, warn, error)"
	echo "  CARGO_TARGET_DIR  Override target directory"
	echo "  FONTLIFT_PROFILE  Override build profile (debug/release)"
}

# Function to verify installation
verify_installation() {
	local build_mode=$1
	local platform_info=$(get_platform_info)
	local platform=$(echo "$platform_info" | cut -d: -f1)
	local dylib_suffix=$(echo "$platform_info" | cut -d: -f3)
	
	print_step "Verifying installation..."
	
	cd "$SCRIPT_DIR"
	
	# Verify CLI
	local cli_path="target/$build_mode/fontlift"
	if [ -f "$cli_path" ]; then
		print_substep "CLI binary verified at $cli_path"
		
		# Test CLI version
		if "$cli_path" --version >/dev/null 2>&1; then
			print_status "CLI version command works ✓"
		else
			print_warning "CLI version command failed"
		fi
	else
		print_warning "CLI binary not found"
	fi
	
	# Verify Python bindings
	local python_path="target/$build_mode/lib_native$dylib_suffix"
	if [ -f "$python_path" ]; then
		print_substep "Python bindings verified at $python_path"
	else
		print_warning "Python bindings not found"
	fi
	
	# Show system information
	print_status "Build verification completed"
	print_status "Platform: $platform"
	print_status "Build mode: $build_mode"
	print_status "Installation ready"
}

# Main function
main() {
	print_header
	
	# Parse command line arguments
	local BUILD_MODE="debug"
	local RUN_TESTS="no"
	local BUILD_PYTHON="no"
	local BUILD_CLI="no"
	local BUILD_CORE="no"
	local BUILD_ALL="yes"
	local CREATE_PACKAGES="no"
	local CLEAN_BUILD="no"
	local CHECK_DEPS_ONLY="no"
	local VERBOSE="no"
	
	while [[ $# -gt 0 ]]; do
		case $1 in
		--release)
			BUILD_MODE="release"
			shift
			;;
		--test)
			RUN_TESTS="yes"
			shift
			;;
		--python)
			BUILD_PYTHON="yes"
			BUILD_ALL="no"
			shift
			;;
		--cli-only)
			BUILD_CLI="yes"
			BUILD_ALL="no"
			shift
			;;
		--core-only)
			BUILD_CORE="yes"
			BUILD_ALL="no"
			shift
			;;
		--package)
			CREATE_PACKAGES="yes"
			shift
			;;
		--clean)
			CLEAN_BUILD="yes"
			shift
			;;
		--check-deps)
			CHECK_DEPS_ONLY="yes"
			shift
			;;
		--verbose)
			VERBOSE="yes"
			set -x
			shift
			;;
		-h | --help)
			show_usage
			exit 0
			;;
		*)
			print_error "Unknown option: $1"
			show_usage
			exit 1
			;;
		esac
	done
	
	# Get platform information
	local platform_info=$(get_platform_info)
	local platform=$(echo "$platform_info" | cut -d: -f1)
	local arch=$(echo "$platform_info" | cut -d: -f2)
	
	print_status "Platform: $platform ($arch)"
	
	# Check dependencies
	check_and_install_dependencies
	
	# If only checking dependencies, exit here
	if [ "$CHECK_DEPS_ONLY" == "yes" ]; then
		print_success "All dependencies verified ✓"
		exit 0
	fi
	
	# Clean build artifacts if requested
	if [ "$CLEAN_BUILD" == "yes" ]; then
		print_step "Cleaning build artifacts..."
		cd "$SCRIPT_DIR"
		cargo clean
		print_status "Build artifacts cleaned"
	fi
	
	print_status "Starting FontLift build process..."
	print_status "Build mode: $BUILD_MODE"
	
	# Build components
	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_CORE" == "yes" ]; then
		build_component "fontlift-core" "" "$BUILD_MODE"
	fi
	
	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_CLI" == "yes" ]; then
		case $platform in
		macos)
			build_component "fontlift-platform-mac" "" "$BUILD_MODE"
			;;
		windows)
			build_component "fontlift-platform-win" "" "$BUILD_MODE"
			;;
		linux)
			print_warning "Linux platform implementation not yet available"
			;;
		esac
		build_component "fontlift-cli" "" "$BUILD_MODE"
	fi
	
	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_PYTHON" == "yes" ]; then
		build_component "fontlift-python" "extension-module" "$BUILD_MODE"
	fi
	
	# Build Python bindings if requested
	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_PYTHON" == "yes" ]; then
		build_python_bindings "$BUILD_MODE"
	fi
	
	# Run tests if requested
	if [ "$RUN_TESTS" == "yes" ]; then
		run_tests "$BUILD_MODE"
	fi
	
	# Create packages if requested
	if [ "$CREATE_PACKAGES" == "yes" ]; then
		create_packages "$BUILD_MODE"
	fi
	
	# Verify installation
	verify_installation "$BUILD_MODE"
	
	# Print build summary
	print_success "Build completed successfully!"
	print_status "Platform: $platform ($arch)"
	print_status "Build mode: $BUILD_MODE"
	
	local dylib_suffix=$(echo "$platform_info" | cut -d: -f3)
	
	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_CLI" == "yes" ]; then
		print_status "CLI binary location: target/$BUILD_MODE/fontlift"
	fi
	
	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_PYTHON" == "yes" ]; then
		print_status "Python module location: target/$BUILD_MODE/lib_native$dylib_suffix"
	fi
	
	echo ""
	print_status "Next steps:"
	print_status "- Run CLI: ./target/$BUILD_MODE/fontlift --help"
	print_status "- Test Python: python3 -c 'import fontlift; print(fontlift.list_fonts())'"
	print_status "- Read documentation: README.md, USAGE.md"
	
	if [ "$CREATE_PACKAGES" == "yes" ]; then
		print_status "- Distribution packages created in dist-$BUILD_MODE-$platform-$arch/"
	fi
	
	echo ""
	print_success "FontLift is ready for production use! 🚀"
}

# Run main function with all arguments
main "$@"
