#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
universal=0
cmake_args=()
for arg in "$@"; do
    if [[ "$arg" == "--universal" ]]; then
        universal=1
    else
        cmake_args+=("$arg")
    fi
done
cargo_cmd=(cargo)
arch_args=(-DCMAKE_OSX_ARCHITECTURES="$(uname -m)" -DCARGO_EXECUTABLE="$(command -v cargo)")
if (( universal )); then
    rustup_bin="${REBIT_RUSTUP:-$(command -v rustup || true)}"
    if [[ -z "$rustup_bin" && -x "$(brew --prefix rustup 2>/dev/null)/bin/rustup" ]]; then
        rustup_bin="$(brew --prefix rustup)/bin/rustup"
    fi
    if [[ -z "$rustup_bin" ]]; then
        printf 'Universal builds need rustup. Install it, then run:\n'
        printf '  rustup toolchain install 1.97.1 --profile minimal --target aarch64-apple-darwin,x86_64-apple-darwin\n'
        exit 1
    fi
    toolchain="${REBIT_RUST_TOOLCHAIN:-1.97.1}"
    for target in aarch64-apple-darwin x86_64-apple-darwin; do
        if ! "$rustup_bin" target list --toolchain "$toolchain" --installed | grep -qx "$target"; then
            printf 'Missing Rust target %s for toolchain %s. Run:\n' "$target" "$toolchain"
            printf '  %s target add --toolchain %s %s\n' "$rustup_bin" "$toolchain" "$target"
            exit 1
        fi
    done
    mkdir -p build
    cargo_wrapper="$PWD/build/rustup-cargo.sh"
    rustc_bin="$("$rustup_bin" which --toolchain "$toolchain" rustc)"
    printf '#!/bin/sh\nexec env RUSTC="%s" "%s" run "%s" cargo "$@"\n' "$rustc_bin" "$rustup_bin" "$toolchain" > "$cargo_wrapper"
    chmod +x "$cargo_wrapper"
    cargo_cmd=("$cargo_wrapper")
    arch_args=(-DCMAKE_OSX_ARCHITECTURES="arm64;x86_64" -DCARGO_EXECUTABLE="$cargo_wrapper")
fi
"${cargo_cmd[@]}" test --offline --locked
cmake -S . -B build -DCMAKE_BUILD_TYPE=Release \
    -DJUCE_PATH="${REBIT_JUCE_PATH:-$PWD/vendor/JUCE}" \
    "${arch_args[@]}" ${cmake_args[@]+"${cmake_args[@]}"}
cmake --build build --parallel "${REBIT_BUILD_JOBS:-6}"
ctest --test-dir build --output-on-failure
if (( universal )); then
    printf '\nBuilt universal Rebit Rust (arm64 + x86_64) in build/RebitRust_artefacts/Release\n'
else
    printf '\nBuilt Rebit Rust for %s in build/RebitRust_artefacts/Release\n' "$(uname -m)"
fi
