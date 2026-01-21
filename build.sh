#!/bin/bash

set -e

echo "🔨 Building Polymarket Copy Trading Bot..."
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Install from https://rustup.rs/"
    echo ""
    echo "   Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

echo "✅ Rust found: $(rustc --version)"

# Check for pkg-config (needed for some dependencies)
if ! command -v pkg-config &> /dev/null; then
    echo "⚠️  pkg-config not found. Installing..."
    if command -v apt-get &> /dev/null; then
        sudo apt-get update && sudo apt-get install -y pkg-config
    elif command -v yum &> /dev/null; then
        sudo yum install -y pkgconfig
    elif command -v brew &> /dev/null; then
        brew install pkg-config
    fi
fi

# Check if .env exists
if [ ! -f .env ]; then
    echo "⚠️  .env file not found. Copying from .env.example..."
    if [ -f .env.example ]; then
        cp .env.example .env
        echo "📝 Please edit .env with your settings before running the bot"
        echo ""
    else
        echo "❌ .env.example not found!"
        exit 1
    fi
fi

# Clean previous build artifacts if requested
if [ "$1" == "--clean" ]; then
    echo "🧹 Cleaning previous build..."
    cargo clean
fi

# Update Cargo.lock
echo "📦 Updating dependencies..."
cargo update 2>/dev/null || true

# Build in release mode
echo ""
echo "🔨 Building in release mode (this may take a few minutes)..."
echo "   Using rustls for TLS (no OpenSSL required)"
echo ""

CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}

if cargo build --release -j "$CARGO_BUILD_JOBS"; then
    echo ""
    echo "✅ Build successful!"
    echo ""
    echo "📦 Binaries created:"
    echo "   - target/release/polymarket-bot"
    echo "   - target/release/mempool-monitor"
    echo ""
    echo "🚀 To run the bot:"
    echo "   1. Edit .env with your configuration"
    echo "   2. Run: ./target/release/polymarket-bot"
    echo ""
    echo "   Or use cargo:"
    echo "   cargo run --release --bin polymarket-bot"
    echo ""
    echo "🔍 To run the mempool monitor:"
    echo "   ./target/release/mempool-monitor"
    echo ""
else
    echo ""
    echo "❌ Build failed. Check errors above."
    echo ""
    echo "💡 Troubleshooting tips:"
    echo "   1. Make sure you have the latest Rust: rustup update"
    echo "   2. Try cleaning and rebuilding: ./build.sh --clean"
    echo "   3. Check the Cargo.toml for dependency issues"
    echo ""
    exit 1
fi
