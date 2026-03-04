# LP Bonds - Add Liquidity and Mint Bond Documentation

## Table of Contents
1. [PDA Derivation Reference](#pda-derivation-reference)
2. [Devnet Validation Checklist](#devnet-validation-checklist)
3. [Common Failure Causes](#common-failure-causes)
4. [Account Structure Reference](#account-structure-reference)

---

## PDA Derivation Reference

### 1. Protocol Config PDA
```
Seeds: ["config"]
Program: AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w (LP Bonds)
```

**Purpose:** Stores global protocol configuration including:
- Admin authority
- Allowlisted whirlpool address
- Bond counter for analytics

**Derivation Code:**
```typescript
const [configPda, bump] = PublicKey.findProgramAddressSync(
  [Buffer.from("config")],
  PROGRAM_ID
);
```

**Source:** `constants.rs:8` - `pub const CONFIG_SEED: &[u8] = b"config";`

---

### 2. Bond Authority PDA
```
Seeds: ["bond_authority"]
Program: AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w (LP Bonds)
```

**Purpose:** Program-controlled signer for bond NFT minting operations. Used as mint authority for bond NFTs.

**Derivation Code:**
```typescript
const [bondAuthorityPda, bump] = PublicKey.findProgramAddressSync(
  [Buffer.from("bond_authority")],
  PROGRAM_ID
);
```

**Source:** `constants.rs:11` - `pub const BOND_AUTHORITY_SEED: &[u8] = b"bond_authority";`

---

### 3. Position Custody PDA
```
Seeds: ["position_custody", bond_mint.key()]
Program: AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w (LP Bonds)
```

**Purpose:** 
- Stores metadata about custodied whirlpool positions
- Acts as owner of the position NFT token account
- Creates 1:1 mapping between bond NFT and position NFT

**Derivation Code:**
```typescript
const [positionCustodyPda, bump] = PublicKey.findProgramAddressSync(
  [Buffer.from("position_custody"), bondMint.toBuffer()],
  PROGRAM_ID
);
```

**Source:** `constants.rs:14` - `pub const POSITION_CUSTODY_SEED: &[u8] = b"position_custody";`

---

### 4. Whirlpool Position PDA
```
Seeds: ["position", position_mint.key()]
Program: whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc (Orca Whirlpool)
```

**Purpose:** Whirlpool's position account storing liquidity data.

**Derivation Code:**
```typescript
const [whirlpoolPositionPda, bump] = PublicKey.findProgramAddressSync(
  [Buffer.from("position"), positionMint.toBuffer()],
  WHIRLPOOL_PROGRAM_ID
);
```

**Source:** `whirlpool_cpi.rs:get_position_address`

---

### 5. Tick Array PDAs
```
Seeds: ["tick_array", whirlpool.key(), start_tick_index.toString()]
Program: whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc (Orca Whirlpool)
```

**Purpose:** Contains tick data for the whirlpool. Each array holds 88 ticks.

**Calculation:**
```typescript
const ticksInArray = tickSpacing * 88; // TICK_ARRAY_SIZE = 88
const startTickIndex = Math.floor(tickIndex / ticksInArray) * ticksInArray;

const [tickArrayPda, bump] = PublicKey.findProgramAddressSync(
  [
    Buffer.from("tick_array"),
    whirlpool.toBuffer(),
    Buffer.from(startTickIndex.toString()),
  ],
  WHIRLPOOL_PROGRAM_ID
);
```

**Important:** Tick arrays must be pre-initialized. If they don't exist, the transaction will fail.

---

## Devnet Validation Checklist

### Pre-Transaction Checklist

- [ ] **Protocol Initialized**
  - Config PDA exists and contains correct data
  - Bond authority PDA is derived correctly

- [ ] **SOL Balance**
  - User has minimum 0.1 SOL for transaction fees
  - Enough SOL for wrapping (sol_amount parameter)

- [ ] **Token B Account**
  - User has ATA for Token B: `4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2`
  - Balance sufficient if position requires Token B

- [ ] **Whirlpool Validation**
  - Whirlpool address: `8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN`
  - Token A = wSOL: `So11111111111111111111111111111111111111112`
  - Token B = `4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2`

- [ ] **Tick Range**
  - tick_lower < tick_upper
  - Both ticks aligned to tick_spacing
  - Both within bounds: -443636 to 443636

- [ ] **Tick Arrays Exist**
  - Lower tick array PDA exists
  - Upper tick array PDA exists (may be same as lower)

- [ ] **Signers Ready**
  - User keypair
  - Bond mint keypair (generated)
  - Position mint keypair (generated)
  - User wSOL account keypair (generated)

### Post-Transaction Checklist

- [ ] **Bond NFT Created**
  - Bond mint account exists
  - Supply = 1
  - Decimals = 0
  - User holds 1 bond NFT

- [ ] **Position Created**
  - Whirlpool position PDA exists
  - Position mint supply = 1

- [ ] **Position in Custody**
  - Position custody PDA contains correct data
  - Custody's position token account holds position NFT (balance = 1)

- [ ] **Protocol State Updated**
  - Config bond_counter incremented

- [ ] **wSOL Cleaned Up**
  - Temporary wSOL account closed
  - Rent returned to user

---

## Common Failure Causes

### 1. Whirlpool CPI Errors

#### `WhirlpoolNotAllowlisted` (6000)
**Cause:** Whirlpool address doesn't match the hardcoded allowlist
**Solution:** Ensure you're using whirlpool `8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN`

#### `InvalidWhirlpoolProgram` (6001)
**Cause:** Wrong whirlpool program ID passed
**Solution:** Use `whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc`

#### `InvalidTokenVault` (6004)
**Cause:** Token vault addresses don't match whirlpool's actual vaults
**Solution:** Fetch vault addresses from whirlpool account data, don't hardcode

#### `TickOutOfBounds` (6006)
**Cause:** Tick index outside valid range
**Solution:** Ensure ticks are between -443636 and 443636

#### `TickNotAlignedToSpacing` (6007)
**Cause:** Tick index not divisible by tick_spacing
**Solution:** Calculate aligned ticks:
```typescript
const alignedTick = Math.floor(tick / tickSpacing) * tickSpacing;
```

### 2. Tick Array Issues

#### Error: "Tick array not found"
**Cause:** Tick arrays for the selected range haven't been initialized
**Solution:** 
1. Use a different tick range with existing arrays
2. Initialize tick arrays using Orca SDK before calling
3. Check if arrays exist before transaction

#### How to verify tick arrays exist:
```typescript
const tickArrayInfo = await connection.getAccountInfo(tickArrayPda);
if (!tickArrayInfo) {
  throw new Error("Tick array not initialized");
}
```

### 3. Token Account Issues

#### `InvalidTokenOwner` (6008)
**Cause:** Token account owner doesn't match expected owner
**Solution:** Ensure user token accounts are owned by user pubkey

#### `InvalidTokenMint` (6009)
**Cause:** Token account mint doesn't match expected mint
**Solution:** Verify ATA derivation uses correct mint address

### 4. Liquidity Calculation Issues

#### Error: "TokenMaxExceeded" from Whirlpool
**Cause:** Calculated liquidity requires more tokens than token_max allows
**Solution:** 
1. Increase slippage tolerance
2. Recalculate liquidity with more buffer
3. Check current tick vs tick range for correct calculation

#### Two-sided position without Token B
**Cause:** Range includes current tick but user has no Token B
**Solution:**
1. Acquire Token B tokens
2. Use single-sided range (entirely above or below current tick)

### 5. Compute Budget Issues

#### Error: "Program failed to complete"
**Cause:** Transaction ran out of compute units
**Solution:** Add compute budget instruction:
```typescript
ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 })
```

### 6. Account Order Issues

#### Error: "Account not found at index X"
**Cause:** Accounts not in correct order as defined in IDL
**Solution:** Use `accountsPartial()` or `accounts()` with named accounts - don't manually order

### 7. Signer Issues

#### Error: "Missing signature"
**Cause:** Not all required signers were provided
**Required Signers:**
1. `user` - Transaction fee payer
2. `bond_mint` - New keypair for bond NFT mint
3. `position_mint` - New keypair for position NFT mint
4. `user_wsol_account` - New keypair for temporary wSOL account

---

## Account Structure Reference

### Instruction: `add_liquidity_and_mint_bond`

| # | Account Name | Type | Signer | Writable | Description |
|---|--------------|------|--------|----------|-------------|
| 1 | user | Signer | ✓ | ✓ | Transaction payer |
| 2 | wsol_mint | Account | | | Native mint address |
| 3 | token_mint_b | Account | | | Token B mint |
| 4 | bond_authority | UncheckedAccount | | | PDA for minting bonds |
| 5 | bond_mint | Account | ✓ | ✓ | New mint for bond NFT |
| 6 | user_wsol_account | Account | ✓ | ✓ | Temp wSOL account |
| 7 | user_token_b_account | Account | | ✓ | User's Token B ATA |
| 8 | user_bond_account | Account | | ✓ | User's bond ATA |
| 9 | config | Account | | ✓ | Protocol config |
| 10 | position_custody | Account | | ✓ | Custody for position |
| 11 | position_mint | Account | ✓ | ✓ | Position NFT mint |
| 12 | whirlpool_position | UncheckedAccount | | ✓ | Whirlpool position |
| 13 | position_token_account | UncheckedAccount | | ✓ | User's position ATA |
| 14 | custody_position_token_account | Account | | ✓ | Custody's position ATA |
| 15 | whirlpool | UncheckedAccount | | ✓ | Whirlpool address |
| 16 | token_vault_a | UncheckedAccount | | ✓ | Pool's wSOL vault |
| 17 | token_vault_b | UncheckedAccount | | ✓ | Pool's Token B vault |
| 18 | tick_array_lower | UncheckedAccount | | ✓ | Lower tick array |
| 19 | tick_array_upper | UncheckedAccount | | ✓ | Upper tick array |
| 20 | whirlpool_program | UncheckedAccount | | | Orca program |
| 21 | token_program | Program | | | SPL Token |
| 22 | associated_token_program | Program | | | ATA program |
| 23 | system_program | Program | | | System program |
| 24 | rent | Sysvar | | | Rent sysvar |

### Instruction Arguments

| Name | Type | Description |
|------|------|-------------|
| tick_lower_index | i32 | Lower tick boundary |
| tick_upper_index | i32 | Upper tick boundary |
| liquidity_amount | u128 | Liquidity to add |
| token_max_a | u64 | Max wSOL to deposit |
| token_max_b | u64 | Max Token B to deposit |
| sol_amount | u64 | SOL to wrap to wSOL |

---

## Quick Start Commands

```bash
# Navigate to project
cd /path/to/solana-lp-bonds-contracts

# Install dependencies
yarn install

# Run the production script
npx ts-node scripts/prod-add-liquidity-and-mint-bond.ts

# Or with environment variable for wallet
ANCHOR_WALLET=~/.config/solana/id.json npx ts-node scripts/prod-add-liquidity-and-mint-bond.ts
```

## Debugging Tips

1. **Enable debug mode:** Set `DEBUG = true` in the script
2. **Check simulation logs:** Script automatically simulates before sending
3. **Verify PDAs:** Compare derived addresses with Anchor's derivation
4. **Check tick arrays:** Query Solana for tick array existence before tx
5. **Monitor explorer:** Use the printed explorer link to check tx details
