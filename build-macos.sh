#!/bin/bash

# FontLift Build Script for macOS
# Optimized build script for fontlift library and CLI on macOS platforms
# Includes Python bindings, comprehensive testing, and package creation

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
SCRIPT_NAME="FontLift macOS Builder"
VERSION="2.0.0-dev"
MIN_RUST_VERSION="1.75.0"

# Function to print colored output
print_header() {
	echo -e "${PURPLE}=== $SCRIPT_NAME v${VERSION} ===${NC}"
	echo -e "${PURPLE}=====================================${NC}"
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

# Function to check if command exists
command_exists() {
	command -v "$1" >/dev/null 2>&1
}

# Function to compare versions
version_compare() {
	local version1="$1" operator="$2" version2="$3"
	# Simple version comparison using sort -V
	if [[ "$operator" == ">=" ]]; then
		[[ "$(printf '%s\n' "$version1" "$version2" | sort -V | head -n1)" == "$version2" ]]
	elif [[ "$operator" == ">" ]]; then
		[[ "$version1" != "$version2" && "$(printf '%s\n' "$version1" "$version2" | sort -V | head -n1)" == "$version2" ]]
	fi
}

# Function to get dynamic library suffix
get_dylib_suffix() {
	echo ".dylib"
}

# Function to check prerequisites
check_prerequisites() {
	print_step "Checking prerequisites..."

	# Check if we're in the right directory
	if [ ! -f "Cargo.toml" ]; then
		print_error "Cargo.toml not found. Please run this script from the fontlift root directory."
		exit 1
	fi

	# Check for Rust
	if ! command_exists cargo; then
		print_error "Rust/Cargo not found. Please install Rust from https://rustup.rs/"
		echo "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
		exit 1
	fi

	# Check Rust version
	local rust_version
	rust_version=$(rustc --version | cut -d' ' -f2)
	if ! version_compare "$rust_version" ">=" "$MIN_RUST_VERSION"; then
		print_error "Rust version $rust_version is too old. Minimum required: $MIN_RUST_VERSION"
		print_status "Update with: rustup update"
		exit 1
	fi
	print_status "Rust version: $rust_version ✓"

	# Check for Xcode command line tools
	if ! command_exists xcodebuild; then
		print_warning "Xcode command line tools not found. FontLift may not build correctly."
		print_status "Install with: xcode-select --install"
	else
		print_status "Xcode command line tools found ✓"
	fi

	# Check for Python and pip/maturin for Python bindings
	if command_exists python3; then
		print_status "Python 3 found ✓"
		if command_exists pip3; then
			print_status "pip3 found ✓"
		else
			print_warning "pip3 not found. Python bindings will not be built."
		fi

		# Check for maturin
		if command_exists maturin; then
			print_status "maturin found for Python bindings ✓"
		else
			print_warning "maturin not found. Install with: pip3 install maturin"
		fi
	else
		print_warning "Python 3 not found. Python bindings will not be built."
	fi

	# Check for uv (optional but recommended)
	if command_exists uv; then
		print_status "uv found - recommended for Python development ✓"
	else
		print_warning "uv not found. Consider installing for better Python dependency management."
	fi
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

	print_step "Running tests..."

	local test_flags=""
	if [ "$build_mode" == "release" ]; then
		test_flags="--release"
	fi

	# Run unit tests
	cargo test --workspace $test_flags --lib
	if [ $? -eq 0 ]; then
		print_status "Unit tests passed"
	else
		print_error "Unit tests failed"
		exit 1
	fi

	# Run integration tests if they exist
	if [ -d "tests" ]; then
		cargo test --workspace $test_flags --test '*'
		if [ $? -eq 0 ]; then
			print_status "Integration tests passed"
		else
			print_error "Integration tests failed"
			exit 1
		fi
	fi

	# Test CLI functionality
	if [ -f "target/$build_mode/fontlift" ]; then
		print_step "Testing CLI functionality..."
		if ./target/$build_mode/fontlift --help >/dev/null 2>&1; then
			print_status "CLI help command works ✓"
		else
			print_error "CLI help command failed"
			exit 1
		fi
	fi

	# Test Python bindings if built
	if [ -f "target/$build_mode/libfontlift_python.dylib" ]; then
		print_step "Testing Python bindings..."
		if python3 -c "import sys; sys.path.insert(0, 'target/$build_mode'); import fontlift_python; print('✓ Python bindings import successful')" 2>/dev/null; then
			print_status "Python bindings work ✓"
		else
			print_warning "Python bindings test failed (may need to be installed first)"
		fi
	fi
}

# Function to build Python bindings
build_python_bindings() {
	local build_mode=$1

	print_step "Building Python bindings..."

	if ! command_exists maturin; then
		print_warning "maturin not found, skipping Python bindings"
		return
	fi

	if ! command_exists python3; then
		print_warning "Python 3 not found, skipping Python bindings"
		return
	fi

	local build_flags=""
	if [ "$build_mode" == "release" ]; then
		build_flags="--release"
	fi

	# Build Python bindings
	maturin develop $build_flags --features extension-module
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

	print_step "Creating distribution packages..."

	# Create distribution directory
	local dist_dir="dist-$build_mode"
	mkdir -p "$dist_dir"

	# Package CLI binary
	if [ -f "target/$build_mode/fontlift" ]; then
		cp "target/$build_mode/fontlift" "$dist_dir/"
		print_status "CLI binary packaged"
	fi

	# Package Python wheel if maturin is available
	if command_exists maturin && [ "$build_mode" == "release" ]; then
		print_step "Building Python wheel..."
		maturin build --release --out "$dist_dir"
		if [ $? -eq 0 ]; then
			print_status "Python wheel created"
		else
			print_warning "Failed to create Python wheel"
		fi
	fi

	# Create archive
	local archive_name="fontlift-macos-$build_mode.tar.gz"
	tar -czf "$archive_name" -C "$dist_dir" .
	print_status "Distribution archive created: $archive_name"

	print_success "Packages created in $dist_dir/"
}

# Function to show usage
show_usage() {
	echo "FontLift macOS Build Script"
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
	echo "  --help            Show this help message"
	echo ""
	echo "Examples:"
	echo "  $0 --release --test --python     Full build with tests and Python bindings"
	echo "  $0 --cli-only --test             Quick CLI build with tests"
	echo "  $0 --release --package           Production build with packaging"
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

	# Check prerequisites
	check_prerequisites

	# Clean build artifacts if requested
	if [ "$CLEAN_BUILD" == "yes" ]; then
		print_step "Cleaning build artifacts..."
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
		build_component "fontlift-platform-mac" "" "$BUILD_MODE"
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

	# Print build summary
	print_success "Build completed successfully!"
	print_status "Build mode: $BUILD_MODE"

	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_CLI" == "yes" ]; then
		print_status "CLI binary location: target/$BUILD_MODE/fontlift"
	fi

	if [ "$BUILD_ALL" == "yes" ] || [ "$BUILD_PYTHON" == "yes" ]; then
		local dylib_suffix
		dylib_suffix=$(get_dylib_suffix)
		print_status "Python module location: target/$BUILD_MODE/libfontlift_python$dylib_suffix"
	fi

	echo ""
	print_status "Next steps:"
	print_status "- Run CLI: ./target/$BUILD_MODE/fontlift --help"
	print_status "- Test Python: python3 -c 'import fontlift_python; print(fontlift_python.list())'"
	print_status "- Read documentation: README.md, USAGE.md"

	if [ "$CREATE_PACKAGES" == "yes" ]; then
		print_status "- Distribution packages created in dist-$BUILD_MODE/"
	fi
}

# Run main function
main "$@"
