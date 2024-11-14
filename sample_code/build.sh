#!/bin/bash

# Define the source file
SOURCE_FILE="get_time.c"

# Define output directory
OUTPUT_DIR="./build"

# Create output directory if it doesn't exist
mkdir -p $OUTPUT_DIR

# Define the common targets
declare -A TARGETS=(
  ["linux_x86_64"]="x86_64-linux-gnu"
  ["linux_arm64"]="aarch64-linux-gnu"
  ["windows_x86_64"]="x86_64-windows-gnu"
  ["windows_arm64"]="aarch64-windows-gnu"
  ["macos_x86_64"]="x86_64-macos"
  ["macos_arm64"]="aarch64-macos"
)

# Loop over each target and compile
for target in "${!TARGETS[@]}"; do
    echo "Compiling for $target (${TARGETS[$target]})..."
    zig cc -target "${TARGETS[$target]}" "$SOURCE_FILE" -g3 -gdwarf -o "$OUTPUT_DIR/${target}" || {
        echo "Failed to compile for $target"
    }
done

echo "Compilation complete. Outputs are in the $OUTPUT_DIR directory."
