#!/bin/bash
set -e

echo "=== LP Bonds Protocol Build Script ==="

# Check if rustup is available
if ! command -v rustup &> /dev/null; then
    echo "Error: rustup not found. Please install Rust via rustup."
    echo "Visit: https://rustup.rs"
    exit 1
fi

# Ensure a default toolchain is set
if ! rustup show active-toolchain &> /dev/null; then
    echo "Setting up stable Rust toolchain..."
    rustup default stable
fi

# Check Rust version
echo "Rust version: $(rustc --version)"
echo "Cargo version: $(cargo --version)"

# Check if anchor is installed
if ! command -v anchor &> /dev/null; then
    echo "Error: Anchor CLI not found."
    echo "Install via: cargo install --git https://github.com/coral-xyz/anchor avm --locked --force"
    echo "Then: avm install latest && avm use latest"
    exit 1
fi

echo "Anchor version: $(anchor --version)"

# Build
echo ""
echo "Building LP Bonds program..."
anchor build

echo ""
echo "=== Build Complete ==="
echo "IDL: target/idl/lp_bonds.json"
echo "Program: target/deploy/lp_bonds.so"
