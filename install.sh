#!/bin/bash
# pqcoin Installer - Quantum-Resistant Digital Currency CLI

set -e

echo ""
echo "╔═══════════════════════════════════════════════════════════╗"
echo "║                                                           ║"
echo "║  ██████╗  ██████╗  ██████╗ ██████╗ ██╗███╗   ██╗        ║"
echo "║  ██╔══██╗██╔═══██╗██╔════╝██╔═══██╗██║████╗  ██║        ║"
echo "║  ██████╔╝██║   ██║██║     ██║   ██║██║██╔██╗ ██║        ║"
echo "║  ██╔═══╝ ██║▄▄ ██║██║     ██║   ██║██║██║╚██╗██║        ║"
echo "║  ██║     ╚██████╔╝╚██████╗╚██████╔╝██║██║ ╚████║        ║"
echo "║  ╚═╝      ╚══▀▀═╝  ╚═════╝ ╚═════╝ ╚═╝╚═╝  ╚═══╝        ║"
echo "║                                                           ║"
echo "║         ⚛️  POST-QUANTUM DIGITAL CURRENCY  ⚛️            ║"
echo "║                                                           ║"
echo "╚═══════════════════════════════════════════════════════════╝"
echo ""
echo "⚛️  Installing pqcoin CLI..."
echo ""

# Check if cargo is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust/Cargo is not installed"
    echo ""
    echo "📋 Install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo ""
    echo "After installation, restart your terminal and run this script again."
    exit 1
fi

echo "✓ Rust detected: $(rustc --version)"
echo ""
echo "⚛️  Building pqcoin (this may take a few minutes)..."
echo ""

# Install the binary
cargo install --path .

echo ""
echo "═══════════════════════════════════════════════════════════"
echo "  ✅ pqcoin installed successfully!"
echo "═══════════════════════════════════════════════════════════"
echo ""
echo "📋 Quick Start:"
echo ""
echo "  1. Generate your SPHINCS+ keypair:"
echo "     pqcoin init"
echo ""
echo "  2. See all available commands:"
echo "     pqcoin --help"
echo ""
echo "  3. Register your vault (requires SOL for gas):"
echo "     pqcoin register --wallet-path ~/.config/solana/id.json"
echo ""
echo "⚛️  Quantum security awaits!"
echo ""
