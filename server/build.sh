#!/bin/bash

# An array of the target triples
TARGETS=(
  # "aarch64-apple-darwin"
  # "x86_64-apple-darwin"
  # "x86_64-unknown-linux-gnu"
  # "aarch64-unknown-linux-gnu"
)

# Loop through each target and build
for TARGET in "${TARGETS[@]}"; do
  echo "Building for $TARGET..."
  cargo build --release --target "$TARGET" || exit 1
done

# Handle the windows build separately because it requires a different toolchain
echo "Buliding for x86_64-pc-windows-msvc..."
cargo xwin build --release --target x86_64-pc-windows-msvc || exit 1
echo "Builds completed successfully."
