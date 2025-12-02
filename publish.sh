#!/usr/bin/env bash

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

tag="${1:-}"
if [[ -z "$tag" ]]; then
	if git describe --tags --exact-match >/dev/null 2>&1; then
		tag=$(git describe --tags --exact-match)
	else
		echo "Usage: ./publish.sh vX.Y.Z"
		exit 1
	fi
fi

version="${tag#v}"
echo "Publishing FontLift version ${version}"

cd "$ROOT"

echo "Building release artifacts..."
cargo build -p fontlift-cli --release

echo "Publishing crates.io packages..."
cargo publish -p fontlift-core
cargo publish -p fontlift-platform-mac || true
cargo publish -p fontlift-platform-win || true
cargo publish -p fontlift-cli
cargo publish -p fontlift-python

echo "Preparing Python wheel..."
python3 -m pip install --upgrade pip hatchling hatchling-pyo3-plugin hatch-vcs twine fire
hatch build -t wheel

echo "Publishing to PyPI..."
python3 -m twine upload dist/fontlift-*.whl

echo "Done."
