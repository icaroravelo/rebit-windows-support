#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo test --offline --locked
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DJUCE_PATH="${REBIT_JUCE_PATH:-$PWD/vendor/JUCE}" "$@"
cmake --build build --parallel "${REBIT_BUILD_JOBS:-6}"
ctest --test-dir build --output-on-failure
printf '\nBuilt Rebit Rust AU, VST3, and standalone in build/RebitRust_artefacts/Release\n'
