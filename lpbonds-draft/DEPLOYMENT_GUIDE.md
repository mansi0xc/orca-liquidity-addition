# LP Bonds Protocol - Deployment & Testing Guide

This guide walks you through deploying, configuring, and testing the LP Bonds protocol on Solana devnet.

## Prerequisites

Before starting, ensure you have:

- **Solana CLI** installed and configured (`solana --version`)
- **Anchor CLI** installed (`anchor --version`)
- **Node.js** (v18+) and **Yarn** installed
- Admin wallet at `~/.config/solana/id.json` with at least **5 SOL** on devnet
- Solana CLI configured for devnet:
  ```bash
  solana config set --url https://api.devnet.solana.com
  ```

## Overview

The LP Bonds protocol consists of two programs:

| Program | Description |
|---------|-------------|
| **LP Bonds** | Base program for Level 1 bonds - deposits liquidity into Orca Whirlpools and mints bond NFTs |
| **LP Bonds Evolution** | Evolution program for upgrading bonds from Level 1 → Level 2 → Level 3 → Level 4 |

---

## Step 1: Deploy Programs

Deploy both programs using the admin wallet.

```bash
yarn deploy:fresh
```

This script will:
1. Generate new program keypairs
2. Update program IDs in source files
3. Build the programs
4. Deploy to devnet
5. Verify deployment

**Expected output:**
```
LP Bonds Program:   <new-program-id>
Evolution Program:  <new-program-id>
Deploy Authority:   <your-admin-pubkey>
```

> **Note:** Save these program IDs - you'll need them for verification.

---

## Step 2: Set Up Tokens and Whirlpools

You have two options:

### Option A: Use Existing Tokens (Recommended for Testing)

Contact **Mansi** to get access to the existing devnet tokens and whirlpools already configured for testing.

**Existing token addresses:**
- **GMI Token (Level 1 Token B):** `4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2`
- **Level 2 Token B:** `Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB`
- **Level 3 Token B:** `9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp`
- **Level 4 Token B:** `9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu`

### Option B: Create Your Own Tokens

If you want to create your own tokens and whirlpools:

#### 2.1 Create SPL Tokens

Create tokens for each level (GMI token + 4 layer tokens):

```bash
# Create a new token
spl-token create-token

# Create a token account for your wallet
spl-token create-account <TOKEN_ADDRESS>

# Mint tokens to your account
spl-token mint <TOKEN_ADDRESS> <AMOUNT>

# Verify your token accounts
spl-token accounts
```

**Example - Create GMI Token:**
```bash
# Create the token mint
spl-token create-token
# Output: Creating token ABC123...

# Create your account for this token
spl-token create-account ABC123...

# Mint 1 billion tokens (with 9 decimals)
spl-token mint ABC123... 1000000000
```

Repeat this for each level's Token B.

#### 2.2 Create Whirlpools on Orca

After creating your tokens:

1. Go to [Orca Devnet](https://devnet.orca.so)
2. Create concentrated liquidity pools pairing:
   - **Level 1:** wSOL / GMI Token
   - **Level 2:** GMI Token / Level 2 Token
   - **Level 3:** GMI Token / Level 3 Token
   - **Level 4:** GMI Token / Level 4 Token
3. Note down each whirlpool address

#### 2.3 Update Configuration

Edit `scripts/configure-bonds.ts` with your token and whirlpool addresses:

```typescript
// Level 1
const LEVEL_1_WHIRLPOOL = new PublicKey("YOUR_LEVEL_1_WHIRLPOOL");
const LEVEL_1_TOKEN_B = new PublicKey("YOUR_GMI_TOKEN");

// Level 2-4
const LEVEL_CONFIGS = [
  {
    level: 2,
    whirlpool: new PublicKey("YOUR_LEVEL_2_WHIRLPOOL"),
    tokenMintB: new PublicKey("YOUR_LEVEL_2_TOKEN_B"),
    // ... other config
  },
  // ... levels 3 and 4
];
```

---

## Step 3: Configure the Protocol

Run the configuration script to initialize all PDAs and configure evolution levels:

```bash
yarn configure:bonds
```

This script will:
1. Initialize LP Bonds base protocol (Level 1 config)
2. Initialize Oracle
3. Initialize Evolution Config
4. Initialize Layer Token Authority PDA
5. Configure Evolution Levels 2-4
6. Verify all configuration

**Expected output:**
```
STEP 1: Initialize LP Bonds Base Protocol
  [OK] https://explorer.solana.com/tx/...

STEP 2: Initialize Oracle
  [OK] https://explorer.solana.com/tx/...

...

CONFIGURATION COMPLETE
```

> **Note:** This script is idempotent - safe to run multiple times. It skips already-initialized accounts.

---

## Step 4: Create Test Wallet

Generate a separate test wallet for user testing (simulates a real user):

```bash
yarn test:generate-wallet
```

**Output:**
```
New test wallet generated:
  Address: <TEST_USER_PUBKEY>
  Path:    ./keys/test-user-wallet.json
```

### Fund the Test Wallet

The test wallet needs:

1. **SOL** for transaction fees (~2 SOL recommended)
2. **Token A** (GMI token) for evolution deposits

**Fund with SOL:**
```bash
solana transfer <TEST_USER_PUBKEY> 2 --allow-unfunded-recipient
```

**Fund with tokens:**
```bash
# Transfer GMI tokens to test user
spl-token transfer <GMI_TOKEN_ADDRESS> <AMOUNT> <TEST_USER_PUBKEY> --fund-recipient
```

---

## Step 5: Transfer Mint Authority (If Using Custom Tokens)

> **Skip this step if using existing tokens from Mansi.**

If you created your own SPL tokens, you need to transfer mint authority to the Layer Token Authority PDA so the protocol can mint layer tokens during evolution:

```bash
yarn admin:transfer-mint-auth
```

This transfers mint authority of Level 2-4 Token B mints to the evolution program's PDA.

---

## Step 6: Run the Full Test Flow

Execute the complete user test flow - mints a Level 1 bond and evolves it to Level 4:

```bash
yarn test:user-flow
```

**What this does:**

| Test | Description |
|------|-------------|
| **Test 1** | Initialize nonce accounts for replay protection |
| **Test 2** | Mint Level 1 bond (deposit SOL into Orca Whirlpool) |
| **Test 3** | Evolve to Level 2 |
| **Test 4** | Evolve to Level 3 |
| **Test 5** | Evolve to Level 4 |

**Expected output:**
```
TEST 2: Mint Level 1 Bond
  Level 1 bond minted: https://explorer.solana.com/tx/...
  Bond NFT mint:       <BOND_MINT>
  Custody level:       1

TEST 3: Evolve to Level 2
  Evolved to Level 2: https://explorer.solana.com/tx/...
  New bond mint:     <NEW_BOND_MINT>
  Custody level:      2

...

ALL TESTS PASSED
```

---

## Troubleshooting

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `AccountNotInitialized` | Token account doesn't exist | Ensure test user has token accounts for all required tokens |
| `InsufficientFunds` | Not enough tokens | Fund test wallet with more tokens |
| `InvalidCustodyPda` | Wrong source custody derivation | Check that evolved bonds use evolution program ID for custody derivation |
| `TokenMaxExceeded` | Slippage too low | Increase `token_max_a` and `token_max_b` in evolution call |

### Verify Token Balances

```bash
# Check test user's token accounts
spl-token accounts --owner <TEST_USER_PUBKEY>
```

### Verify Configuration

```bash
# Re-run configure to verify (skips already-initialized accounts)
yarn configure:bonds
```

### Check Transaction on Explorer

All successful transactions print Solana Explorer links. Use these to debug failed transactions.

---

## Quick Reference - All Commands

```bash
# 1. Deploy programs
yarn deploy:fresh

# 2. Configure protocol
yarn configure:bonds

# 3. Generate test wallet
yarn test:generate-wallet

# 4. Transfer mint authority (custom tokens only)
yarn admin:transfer-mint-auth

# 5. Run full test flow
yarn test:user-flow
```

You can refer to the folder named version-optimization to view the outputs of all commands.
---

## File Structure

```
scripts/
├── deploy-fresh.sh          # Deploys both programs
├── configure-bonds.ts       # Initializes and configures protocol
├── transfer-mint-authority.ts # Transfers mint authority to PDA
├── user-test.ts             # Full user test flow (mint + evolve L1→L4)
└── keys/
    └── test-user-wallet.json # Generated test wallet
```

---

## Support

For questions or issues:
- Contact **Mansi** for access to existing devnet tokens
- Check `version-optimization/` folder for example outputs
- Review transaction logs on [Solana Explorer](https://explorer.solana.com/?cluster=devnet)

