#!/usr/bin/env bash
# Fully deterministic SourceViewer comparison using tempdirs
set -u

SV_CARGO_DIR="$(dirname "$(realpath "$0")")"
TMP_DIR="$(mktemp -d -t sv-compare-XXXXXX)"
OUT_DIR="$TMP_DIR/outputs"
mkdir -p "$OUT_DIR"

SV_BIN_SYS="$(command -v SourceViewer)"
SV_BIN_CARGO="$SV_CARGO_DIR/target/release/SourceViewer"

SAMPLES_DIR="$(realpath "$SV_CARGO_DIR/sample_code")"
BINARIES=(
  "$SAMPLES_DIR/hello-world"
  "$SAMPLES_DIR/llvm-impl/small"
  "$SAMPLES_DIR/llvm-impl/libsmall_lang.so"
)

COMMANDS=(
  "sections"
  "lines"
  "view-source"
  "view-sources"
  "dwarf-dump"
)

declare -i total=0
declare -i same=0
declare -i diff=0

cleanup() {
    echo ""
    echo "🧹 Temporary files are in: $TMP_DIR"
    echo "Delete them manually when done."
}
trap cleanup EXIT

echo "🧱 Building release build..."
cargo build --release --quiet || { echo "❌ Build failed"; exit 1; }
echo "✅ Using binaries:"
echo "   System : $SV_BIN_SYS"
echo "   Cargo  : $SV_BIN_CARGO"
echo "   TempDir: $TMP_DIR"
echo ""

summary_file="$OUT_DIR/diff_summary.txt"
echo "Diff summary (line counts):" > "$summary_file"
echo "============================" >> "$summary_file"

run_tool() {
    local which="$1" cmd="$2" bin="$3" outfile="$4"
    echo "▶ [$which] $cmd on $bin"
    if [[ "$which" == "cargo" ]]; then
        "$SV_BIN_CARGO" "$cmd" "$bin" >"$outfile" 2>&1
    else
        "$SV_BIN_SYS" "$cmd" "$bin" >"$outfile" 2>&1
    fi
}

compare_outputs() {
    local cmd="$1" binpath="$2"
    local binname
    binname="$(basename "$binpath")"
    local cargo_file="$OUT_DIR/cargo_${cmd}_${binname}.txt"
    local sys_file="$OUT_DIR/system_${cmd}_${binname}.txt"
    local diff_file="$OUT_DIR/diff_${cmd}_${binname}.txt"

    ((total++))
    local total_lines
    total_lines=$(wc -l <"$cargo_file" | tr -d ' ')

    if diff -u --color=never "$cargo_file" "$sys_file" >"$diff_file"; then
        echo "✅ Match for $cmd on $binname ($total_lines lines total)"
        ((same++))
        echo "$binname/$cmd : identical ($total_lines lines)" >>"$summary_file"
    else
        ((diff++))
        local diff_lines
        diff_lines=$(grep -cE '^[+-]' "$diff_file" || true)
        echo "⚠️  Diff for $cmd on $binname ($diff_lines of $total_lines lines differ)"
        echo "$binname/$cmd : $diff_lines of $total_lines lines differ" >>"$summary_file"
        if [[ "$diff_lines" -lt 50 ]]; then
            echo "--- Showing first 20 lines of diff ---"
            head -n 20 "$diff_file"
            echo "-------------------------------------"
        fi
    fi
}

cd "$SV_CARGO_DIR" || exit

for binpath in "${BINARIES[@]}"; do
    binpath="$(realpath "$binpath")"
    binname="$(basename "$binpath")"
    echo ""
    echo "=== Testing on $binname ==="
    for cmd in "${COMMANDS[@]}"; do
        run_tool cargo "$cmd" "$binpath" "$OUT_DIR/cargo_${cmd}_${binname}.txt"
        run_tool system "$cmd" "$binpath" "$OUT_DIR/system_${cmd}_${binname}.txt"
        compare_outputs "$cmd" "$binpath"
    done
done

echo ""
echo "===== Summary ====="
echo "Total comparisons: $total"
echo "Identical:         $same"
echo "Differing:         $diff"
echo "==================="
echo ""
echo "Detailed summary written to: $summary_file"
cat "$summary_file"
