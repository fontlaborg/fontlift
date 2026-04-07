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

echo "Syncing Cargo.toml versions to ${version}..."
# Update workspace version
sed -i '' "s/^version = \"[0-9]*\.[0-9]*\.[0-9]*\"/version = \"${version}\"/" Cargo.toml
# Update internal dependency version pins (=X.Y.Z)
sed -i '' "s/version = \"=[0-9]*\.[0-9]*\.[0-9]*\"/version = \"=${version}\"/g" Cargo.toml

git add Cargo.toml
git commit -m "sync Cargo.toml to ${version}"
git tag -f "$tag"
git push origin main
git push origin "$tag" --force

echo "Building release artifacts..."
cargo build -p fontlift-cli --release

echo "Publishing crates.io packages..."
cargo publish -p fontlift-core
cargo publish -p fontlift-platform-mac || true
cargo publish -p fontlift-platform-win || true
cargo publish -p fontlift-cli
cargo publish -p fontlift-python

echo "Preparing Python wheel..."
uvx maturin build --release

echo "Publishing to PyPI..."
uv publish target/wheels/fontlift-${version}*.whl

echo "Done."
