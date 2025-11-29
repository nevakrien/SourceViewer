#!/usr/bin/env bash
set -u

SV_CARGO_DIR="$(dirname "$(realpath "$0")")"
SV_BIN_SYS="$(command -v SourceViewer)"
SV_BIN_CARGO="$SV_CARGO_DIR/target/release/SourceViewer"

# BIN="$SV_CARGO_DIR/sample_code/llvm-impl/small"
BIN="$SV_CARGO_DIR/sample_code/llvm-impl/libsmall_lang.so"
BIN="$(realpath "$BIN")"

COMMANDS=(
  "sections"
  "lines"
)

echo "=== Testing on small ==="
echo "Binary: $BIN"
echo "Cargo  : $SV_BIN_CARGO"
echo "System : $SV_BIN_SYS"
echo

for cmd in "${COMMANDS[@]}"; do
    echo "▶ [cargo]  $cmd"
    cargo_out="$(mktemp)"
    "$SV_BIN_CARGO" "$cmd" "$BIN" >"$cargo_out" 2>&1

    echo "▶ [system] $cmd"
    system_out="$(mktemp)"
    "$SV_BIN_SYS" "$cmd" "$BIN" >"$system_out" 2>&1

    echo
    echo "====== FULL DIFF for '$cmd' on small ======"
    diff --color=always -u "$cargo_out" "$system_out"
    echo "============================================"
    echo

    rm -f "$cargo_out" "$system_out"
done
