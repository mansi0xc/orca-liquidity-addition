#!/usr/bin/env bash
#
# Fresh deployment script for LP Bonds Protocol v2
#
# Generates new program keypairs, updates source files, builds, and deploys.
# Uses ~/.config/solana/id.json as the deploy authority.
#
# Usage:
#   chmod +x scripts/deploy-fresh.sh
#   ./scripts/deploy-fresh.sh
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$ROOT_DIR"

echo "========================================================================"
echo "LP BONDS PROTOCOL - FRESH DEPLOYMENT"
echo "========================================================================"
echo ""

WALLET="${ANCHOR_WALLET:-$HOME/.config/solana/id.json}"
CLUSTER="${ANCHOR_PROVIDER_URL:-https://api.devnet.solana.com}"

echo "Wallet:  $WALLET"
echo "Cluster: $CLUSTER"
echo ""

ADMIN_PUBKEY=$(solana-keygen pubkey "$WALLET")
echo "Admin:   $ADMIN_PUBKEY"

BALANCE=$(solana balance --keypair "$WALLET" --url "$CLUSTER" | awk '{print $1}')
echo "Balance: $BALANCE SOL"
echo ""

# =========================================================================
# STEP 1: Generate new program keypairs
# =========================================================================

echo "--- Step 1: Generate new program keypairs ---"

LP_BONDS_KP="target/deploy/lp_bonds-keypair.json"
EVOLUTION_KP="target/deploy/lp_bonds_evolution-keypair.json"

mkdir -p target/deploy

solana-keygen new -o "$LP_BONDS_KP" --force --no-bip39-passphrase --silent
solana-keygen new -o "$EVOLUTION_KP" --force --no-bip39-passphrase --silent

LP_BONDS_ID=$(solana-keygen pubkey "$LP_BONDS_KP")
EVOLUTION_ID=$(solana-keygen pubkey "$EVOLUTION_KP")

echo "New LP Bonds Program ID:  $LP_BONDS_ID"
echo "New Evolution Program ID: $EVOLUTION_ID"
echo ""

# =========================================================================
# STEP 2: Update program IDs in source files
# =========================================================================

echo "--- Step 2: Update program IDs in source ---"

# Update declare_id! in lp-bonds lib.rs
sed -i.bak "s/declare_id!(\"[A-Za-z0-9]*\")/declare_id!(\"$LP_BONDS_ID\")/" \
  programs/lp-bonds/src/lib.rs
rm -f programs/lp-bonds/src/lib.rs.bak

# Update declare_id! in lp-bonds-evolution lib.rs
sed -i.bak "s/declare_id!(\"[A-Za-z0-9]*\")/declare_id!(\"$EVOLUTION_ID\")/" \
  programs/lp-bonds-evolution/src/lib.rs
rm -f programs/lp-bonds-evolution/src/lib.rs.bak

# Update Anchor.toml
sed -i.bak "s/^lp_bonds = \"[A-Za-z0-9]*\"/lp_bonds = \"$LP_BONDS_ID\"/" Anchor.toml
sed -i.bak "s/^lp_bonds_evolution = \"[A-Za-z0-9]*\"/lp_bonds_evolution = \"$EVOLUTION_ID\"/" Anchor.toml
rm -f Anchor.toml.bak

echo "Updated declare_id! and Anchor.toml"
echo ""

# =========================================================================
# STEP 3: Build
# =========================================================================

echo "--- Step 3: Build programs ---"
anchor build 2>&1 | grep -E "Compiling|Finished|Error" || true
echo ""

# Verify .so files exist
if [ ! -f "target/deploy/lp_bonds.so" ] || [ ! -f "target/deploy/lp_bonds_evolution.so" ]; then
  echo "ERROR: Build failed - .so files not found"
  exit 1
fi

echo "Build successful."
echo "  lp_bonds.so:           $(wc -c < target/deploy/lp_bonds.so) bytes"
echo "  lp_bonds_evolution.so: $(wc -c < target/deploy/lp_bonds_evolution.so) bytes"
echo ""

# =========================================================================
# STEP 4: Deploy
# =========================================================================

echo "--- Step 4: Deploy programs ---"

echo "Deploying lp_bonds..."
solana program deploy \
  target/deploy/lp_bonds.so \
  --keypair "$WALLET" \
  --url "$CLUSTER" \
  --program-id "$LP_BONDS_KP" \
  --with-compute-unit-price 5000

echo ""
echo "Deploying lp_bonds_evolution..."
solana program deploy \
  target/deploy/lp_bonds_evolution.so \
  --keypair "$WALLET" \
  --url "$CLUSTER" \
  --program-id "$EVOLUTION_KP" \
  --with-compute-unit-price 5000

echo ""

# =========================================================================
# STEP 5: Verify
# =========================================================================

echo "--- Step 5: Verify deployment ---"

solana program show "$LP_BONDS_ID" --url "$CLUSTER" 2>&1 | head -5
echo ""
solana program show "$EVOLUTION_ID" --url "$CLUSTER" 2>&1 | head -5

echo ""
echo "========================================================================"
echo "DEPLOYMENT COMPLETE"
echo "========================================================================"
echo ""
echo "LP Bonds Program:   $LP_BONDS_ID"
echo "Evolution Program:   $EVOLUTION_ID"
echo "Deploy Authority:    $ADMIN_PUBKEY"
echo ""
echo "Next step: Run the admin configuration script:"
echo "  npx ts-node scripts/admin-configure.ts"
echo ""
echo "========================================================================"
