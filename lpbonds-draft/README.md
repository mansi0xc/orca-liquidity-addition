# LP Bonds Protocol

Production-grade Anchor program for Orca Whirlpool position custody and Bond NFT minting.

## Architecture Overview

**PDA Structure:**
- `ProtocolConfig` (`["config"]`): Stores allowlisted whirlpool, admin, bond counter
- `PositionCustody` (`["position_custody", bond_mint]`): Holds position metadata, owns position NFT
- `BondAuthority` (`["bond_authority"]`): Signs bond NFT mint operations (no data)

**Authority Model:**
- Admin authority initializes protocol and can update config
- Bond authority PDA signs minting operations - cannot be transferred or spoofed
- Position custody PDA owns whirlpool position NFTs - only program can transfer

**Position NFT Custody:**
- Whirlpool position NFT transferred to PDA-owned token account on creation
- 1:1 mapping between bond mint and custody PDA via seeds
- Position NFT released to user on bond redemption

**Bond NFT Mint Authority:**
- Each bond is unique SPL token (decimals=0, supply=1)
- Bond authority PDA is mint authority - allows programmatic minting
- Bond represents ownership claim on underlying position

## Complete Account List

### Initialize

| Account | mut | signer | Owner Validation | PDA Seeds | Constraints |
|---------|-----|--------|------------------|-----------|-------------|
| `admin` | ✓ | ✓ | - | - | Payer |
| `config` | ✓ | - | Program | `["config"]` | Init |
| `bond_authority` | - | - | Program | `["bond_authority"]` | - |
| `system_program` | - | - | Native | - | - |

### AddLiquidityAndMintBond

| Account | mut | signer | Owner Validation | PDA Seeds | Constraints |
|---------|-----|--------|------------------|-----------|-------------|
| `user` | ✓ | ✓ | - | - | Payer, SOL provider |
| `user_wsol_account` | ✓ | - | Token Program | - | Init, authority=user |
| `user_token_b_account` | ✓ | - | user.key() | - | mint=token_mint_b |
| `user_bond_account` | ✓ | - | ATA | - | ATA(bond_mint, user) |
| `config` | ✓ | - | Program | `["config"]` | bump=config.bump |
| `bond_authority` | - | - | Program | `["bond_authority"]` | - |
| `position_custody` | ✓ | - | Program | `["position_custody", bond_mint]` | Init |
| `bond_mint` | ✓ | ✓ | Token Program | - | Init, authority=bond_authority |
| `bond_metadata` | ✓ | - | Metaplex | `["metadata", MPL, bond_mint]` | Created via CPI |
| `position_mint` | ✓ | ✓ | - | - | New keypair |
| `whirlpool_position` | ✓ | - | Whirlpool | `["position", position_mint]` | Created via CPI |
| `position_token_account` | ✓ | - | Whirlpool | - | Created via CPI |
| `whirlpool` | ✓ | - | Whirlpool | - | == ALLOWLISTED_WHIRLPOOL |
| `token_vault_a` | ✓ | - | Whirlpool | - | == whirlpool.token_vault_a |
| `token_vault_b` | ✓ | - | Whirlpool | - | == whirlpool.token_vault_b |
| `tick_array_lower` | ✓ | - | Whirlpool | - | Validated by Whirlpool |
| `tick_array_upper` | ✓ | - | Whirlpool | - | Validated by Whirlpool |
| `wsol_mint` | - | - | Token Program | - | == NATIVE_MINT |
| `token_mint_b` | - | - | Token Program | - | == EXPECTED_TOKEN_MINT_B |
| `whirlpool_program` | - | - | - | - | == WHIRLPOOL_PROGRAM_ID |
| `token_program` | - | - | Native | - | - |
| `associated_token_program` | - | - | Native | - | - |
| `metadata_program` | - | - | Native | - | - |
| `system_program` | - | - | Native | - | - |
| `rent` | - | - | Native | - | - |

### RedeemBond

| Account | mut | signer | Owner Validation | PDA Seeds | Constraints |
|---------|-----|--------|------------------|-----------|-------------|
| `user` | ✓ | ✓ | - | - | - |
| `user_bond_account` | ✓ | - | user.key() | - | mint=bond_mint, amount=1 |
| `user_position_token_account` | ✓ | - | ATA | - | ATA(position_mint, user) |
| `bond_mint` | ✓ | - | Token Program | - | - |
| `position_mint` | - | - | Token Program | - | == position_custody.position_mint |
| `position_custody` | ✓ | - | Program | `["position_custody", bond_mint]` | bump=position_custody.bump |
| `custody_position_token_account` | ✓ | - | position_custody | - | mint=position_mint, amount=1 |
| `token_program` | - | - | Native | - | - |
| `associated_token_program` | - | - | Native | - | - |
| `system_program` | - | - | Native | - | - |

## Instruction Execution Order

### add_liquidity_and_mint_bond

```
1. VALIDATION PHASE
   ├─ Verify whirlpool == ALLOWLISTED_WHIRLPOOL
   ├─ Verify whirlpool_program == WHIRLPOOL_PROGRAM_ID
   ├─ Validate tick_lower < tick_upper
   ├─ Validate MIN_TICK <= ticks <= MAX_TICK
   ├─ Validate tick % tick_spacing == 0
   ├─ Verify whirlpool.token_mint_a == EXPECTED_TOKEN_MINT_A
   ├─ Verify whirlpool.token_mint_b == EXPECTED_TOKEN_MINT_B
   └─ Validate sol_amount > 0

2. WRAP SOL TO wSOL
   ├─ system_instruction::transfer(user -> user_wsol_account, sol_amount)
   └─ token::sync_native(user_wsol_account)

3. OPEN WHIRLPOOL POSITION (CPI)
   ├─ whirlpool::open_position
   ├─ Position owned by position_custody PDA
   └─ Position NFT held in position_token_account

4. INCREASE LIQUIDITY (CPI)
   ├─ whirlpool::increase_liquidity
   ├─ Transfers tokens from user accounts to vaults
   └─ User signs as position_authority

5. MINT BOND NFT
   ├─ token::mint_to(bond_mint -> user_bond_account, 1)
   ├─ Signed by bond_authority PDA
   └─ create_metadata_accounts_v3 (Metaplex CPI)

6. UPDATE PROTOCOL STATE
   ├─ Store position info in position_custody
   └─ Increment config.bond_counter

7. CLEANUP
   ├─ token::close_account(user_wsol_account)
   └─ Return remaining lamports + rent to user

8. EMIT EVENT
   └─ emit!(BondMinted { ... })
```

### redeem_bond

```
1. VALIDATION
   └─ Verify user_bond_account.amount == 1

2. BURN BOND NFT
   └─ token::burn(bond_mint, 1)

3. TRANSFER POSITION NFT
   ├─ token::transfer(custody -> user)
   └─ Signed by position_custody PDA

4. EMIT EVENT
   └─ emit!(BondRedeemed { ... })
```

## Security Review

### 1. Fake Whirlpool Injection
**Attack:** Attacker provides malicious whirlpool account to steal tokens.
**Defense:**
- Hardcoded `ALLOWLISTED_WHIRLPOOL` constant in `constants.rs`
- Constraint: `whirlpool.key() == ALLOWLISTED_WHIRLPOOL`
- Runtime check in instruction: `require_keys_eq!(...)`

### 2. Signer Escalation
**Attack:** Attacker gains unauthorized signing privileges via CPI.
**Defense:**
- PDA signers scoped to specific operations
- `bond_authority` only signs mint/metadata operations
- `position_custody` only signs position transfers
- User must sign all value-transferring operations

### 3. Token Account Substitution
**Attack:** Attacker substitutes malicious token accounts.
**Defense:**
- Owner validation: `constraint = account.owner == expected`
- Mint validation: `constraint = account.mint == expected_mint`
- Vault validation: `constraint = vault == whirlpool.token_vault_a`

### 4. PDA Spoofing
**Attack:** Attacker provides fake PDA to bypass checks.
**Defense:**
- All PDAs derived with `seeds` and `bump` constraints
- External PDAs validated with `seeds::program`
- Bumps stored in accounts for subsequent operations

### 5. Metadata Forgery
**Attack:** Attacker creates fake bond with spoofed metadata.
**Defense:**
- Bond authority PDA is mint authority
- Metadata created only via program CPI
- Cannot mint/create metadata without program control

### 6. CPI Privilege Leaks
**Attack:** Attacker exploits CPI to gain elevated privileges.
**Defense:**
- `CpiContext::new_with_signer` used minimally
- Signer seeds scoped to single PDA
- No authority delegation to external accounts

### 7. Tick Array Manipulation
**Attack:** Attacker provides invalid tick arrays.
**Defense:**
- Tick bounds checked: `MIN_TICK_INDEX <= tick <= MAX_TICK_INDEX`
- Tick spacing alignment validated
- Whirlpool program validates tick arrays during CPI

### 8. Reentrancy
**Defense:**
- Solana runtime prevents reentrancy
- State updates occur after external calls (checks-effects-interactions pattern)
- Atomic transaction execution

## Compute Budget & Transaction Size

### Compute Units
- **Estimated:** 400,000 - 600,000 CU
- **Recommendation:** Include `ComputeBudget::set_compute_unit_limit(600_000)`

**Breakdown:**
- SOL wrapping + sync_native: ~5,000 CU
- Whirlpool open_position CPI: ~150,000 CU
- Whirlpool increase_liquidity CPI: ~100,000 CU
- Bond NFT minting: ~10,000 CU
- Metaplex metadata creation: ~100,000 CU
- Account closures + state updates: ~20,000 CU
- Buffer for safety: ~200,000 CU

### Transaction Size
- **Estimated:** ~1,100 bytes
- **Limit:** 1,232 bytes
- **Status:** Within limits, but close

**Account count:** 25+ accounts
**Recommendation:** Pre-create user ATAs in separate transaction if needed.

### Priority Fees
For mainnet congestion, include:
```rust
ComputeBudget::set_compute_unit_price(priority_fee_microlamports)
```

## Usage

### Build
```bash
anchor build
```

### Test
```bash
# Start local validator with cloned mainnet state
anchor test --skip-local-validator

# Or with local validator
anchor test
```

### Deploy
```bash
anchor deploy --provider.cluster devnet
```

## Configuration

### Allowlisted Whirlpool
Hardcoded in `src/constants.rs`:
```rust
pub const ALLOWLISTED_WHIRLPOOL: Pubkey = 
    pubkey!("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");
```

### Expected Token Mints
```rust
pub const EXPECTED_TOKEN_MINT_A: Pubkey = NATIVE_MINT; // wSOL
pub const EXPECTED_TOKEN_MINT_B: Pubkey = 
    pubkey!("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
```

## Oracle Verification System

The LP Bonds protocol includes a full on-chain oracle verification system for validating position collateral through Ed25519 signatures.

### Architecture

```
Oracle Verification Flow:
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Oracle API    │────▶│  Ed25519 Sign   │────▶│ Canonical Msg   │
│  (Off-chain)    │     │  (tweetnacl)    │     │  (198 bytes)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
         │                                               │
         ▼                                               ▼
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│ Client Builds   │────▶│ Ed25519 Precomp │────▶│ verify_collat.  │
│   Transaction   │     │  Instruction    │     │  Instruction    │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                                │                       │
                                ▼                       ▼
                        ┌─────────────────────────────────────┐
                        │     Solana Runtime Verification     │
                        │   1. Ed25519 sig verified by native │
                        │   2. LP Bonds verifies message      │
                        │   3. Nonce updated for replay prot. │
                        └─────────────────────────────────────┘
```

### New Accounts

#### OracleConfig
PDA Seeds: `["oracle_config"]`
```rust
pub struct OracleConfig {
    pub oracle_authority: Pubkey,  // Ed25519 public key
    pub admin: Pubkey,             // Admin who can update
    pub enabled: bool,             // Whether oracle is active
    pub bump: u8,
}
```

#### NonceAccount
PDA Seeds: `["nonce", user_pubkey]`
```rust
pub struct NonceAccount {
    pub user: Pubkey,              // User this nonce belongs to
    pub current_nonce: u64,        // Strictly increasing counter
    pub bump: u8,
}
```

### New Instructions

#### initialize_oracle
Initializes the oracle configuration with a trusted authority.
```typescript
await program.methods
  .initializeOracle(oracleAuthorityPubkey)
  .accounts({
    admin: adminPubkey,
    config: configPda,
    oracleConfig: oracleConfigPda,
    systemProgram: SystemProgram.programId,
  })
  .signers([admin])
  .rpc();
```

#### update_oracle_authority
Updates the oracle authority (admin only).
```typescript
await program.methods
  .updateOracleAuthority(newOracleAuthorityPubkey)
  .accounts({
    admin: adminPubkey,
    oracleConfig: oracleConfigPda,
  })
  .signers([admin])
  .rpc();
```

#### initialize_nonce
Creates a nonce account for a user.
```typescript
await program.methods
  .initializeNonce()
  .accounts({
    user: userPubkey,
    nonceAccount: noncePda,
    systemProgram: SystemProgram.programId,
  })
  .signers([user])
  .rpc();
```

#### verify_collateral
Verifies oracle-signed position data.
```typescript
// Build Ed25519 instruction first
const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
  publicKey: oraclePublicKey,
  signature: signature,
  message: canonicalMessage,
});

// Build verify_collateral instruction
const verifyIx = await program.methods
  .verifyCollateral(
    amount0, amount1, liquidity,
    tickLower, tickUpper, tickCurrent,
    nonce, signatureArray
  )
  .accounts({
    sender: userPubkey,
    oracleConfig: oracleConfigPda,
    nonceAccount: noncePda,
    bondMint: bondMintPubkey,
    positionCustody: positionCustodyPda,
    instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
  })
  .instruction();

// Send transaction with both instructions
const tx = new Transaction()
  .add(ed25519Ix)
  .add(verifyIx);
```

### Canonical Message Format

The oracle signs a deterministic 198-byte message:

| Offset | Size | Field | Type |
|--------|------|-------|------|
| 0 | 18 | Domain ("LP_BONDS_SOLANA_V1") | UTF-8 |
| 18 | 32 | bond_mint | Pubkey |
| 50 | 32 | position_mint | Pubkey |
| 82 | 8 | amount0 | u64 LE |
| 90 | 8 | amount1 | u64 LE |
| 98 | 16 | liquidity | u128 LE |
| 114 | 4 | tick_lower | i32 LE |
| 118 | 4 | tick_upper | i32 LE |
| 122 | 4 | tick_current | i32 LE |
| 126 | 8 | nonce | u64 LE |
| 134 | 32 | sender | Pubkey |
| 166 | 32 | contract_address | Pubkey |

### Security Guarantees

1. **Replay Attack Prevention**
   - Per-user nonce account with strictly increasing nonces
   - Nonce is part of signed message
   - Same signature cannot be reused

2. **Cross-User Replay Prevention**
   - Sender pubkey is part of signed message
   - Signature from one user cannot be used by another

3. **Cross-Position Replay Prevention**
   - Bond mint and position mint are part of signed message
   - Signature for one position cannot be used for another

4. **Contract Address Binding**
   - Program ID is part of signed message
   - Cannot replay on different contracts

5. **Deterministic Serialization**
   - Fixed-size 198-byte message format
   - No JSON, no floating point
   - Little-endian byte order
   - Exact match between TypeScript and Rust implementations

### Oracle Error Codes

| Code | Name | Description |
|------|------|-------------|
| 6080 | InvalidOracleSignature | Ed25519 signature verification failed |
| 6081 | Ed25519InstructionNotFound | Ed25519 instruction missing in transaction |
| 6082 | InvalidOracleAuthority | Oracle authority mismatch |
| 6083 | NonceAlreadyUsed | Nonce not strictly greater |
| 6084 | NonceTooOld | Nonce is stale |
| 6085 | MessageReconstructionFailed | On-chain message doesn't match |
| 6086 | PositionDataMismatch | Position data doesn't match custody |
| 6087 | OracleAlreadyInitialized | Oracle config exists |
| 6088 | OracleNotInitialized | Oracle not configured |
| 6089 | InvalidMessageLength | Wrong message length |

### Events

#### OracleInitialized
Emitted when oracle is initialized.

#### OracleAuthorityUpdated
Emitted when oracle authority changes.

#### CollateralVerified
Emitted on successful verification.

#### NonceInitialized
Emitted when user's nonce account is created.

#### NonceIncremented
Emitted when nonce is updated.

### Running Oracle Tests

```bash
# Run all tests including oracle verification
anchor test

# Run only oracle verification tests
anchor test -- --grep "Oracle Verification"
```

### Running Verification Script

```bash
# Run the verification demo script
npx ts-node scripts/prod-verify-collateral.ts
```

## Evolution Program

The LP Bonds Evolution program enables users to upgrade their LP bonds through multiple levels (Level 1 → Level 4), increasing their liquidity position and earning enhanced rewards.

### Architecture Overview

```
Evolution Flow:
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Level 1 Bond  │────▶│   Level 2 Bond  │────▶│   Level 3 Bond  │────▶│   Level 4 Bond  │
│  (Base LP Bond) │     │  (Evolved)      │     │  (Evolved)      │     │  (Max Level)    │
└─────────────────┘     └─────────────────┘     └─────────────────┘     └─────────────────┘
        │                       │                       │                       │
        ▼                       ▼                       ▼                       ▼
   Whirlpool L1            Whirlpool L2            Whirlpool L3            Whirlpool L4
```

### Program IDs

| Program | Devnet Address |
|---------|----------------|
| LP Bonds (Base) | `AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w` |
| LP Bonds Evolution | `Bk81YHvFinrSCs64W7MzobDhMJNXrUEqAU5YpAWcotua` |

### Evolution Program PDA Structure

| PDA | Seeds | Description |
|-----|-------|-------------|
| `EvolutionConfig` | `["evolution_config"]` | Global evolution settings, admin, oracle, treasury |
| `LevelConfig` | `["level_config", level_id]` | Per-level configuration (whirlpool, fees, amounts) |
| `LayerTokenAuthority` | `["layer_token_authority"]` | PDA that controls layer token operations |
| `BondAuthority` | `["bond_authority"]` | Signs evolved bond NFT mints |
| `PositionCustody` | `["position_custody", bond_mint]` | Stores position data for evolved bonds |
| `EvolutionRecord` | `["evolution_record", source_bond_mint]` | Tracks evolution history |
| `NonceAccount` | `["nonce", user]` | Per-user replay protection |

### Level Configuration

| Level | Lock Duration | Fee | Multiplier | Required Tokens |
|-------|---------------|-----|------------|-----------------|
| Level 2 | 60 days | 1% | 1.5x | 2B Token A, 1B Token B |
| Level 3 | 90 days | 1.5% | 2x | 3B Token A, 1.5B Token B |
| Level 4 | 120 days | 2% | 3x | 4B Token A, 2B Token B |

### Evolution Instructions

#### initialize_evolution
Initializes the evolution program configuration.

```typescript
await evolutionProgram.methods
  .initializeEvolution(treasury, oracleAuthority)
  .accounts({ admin: adminPubkey })
  .rpc();
```

#### initialize_layer_authority
Creates the layer token authority PDA.

```typescript
await evolutionProgram.methods
  .initializeLayerAuthority()
  .accounts({ admin: adminPubkey })
  .rpc();
```

#### configure_level
Configures a specific evolution level with its whirlpool and parameters.

```typescript
await evolutionProgram.methods
  .configureLevel(
    level,           // u8: 2, 3, or 4
    tickLower,       // i32: lower tick bound
    tickUpper,       // i32: upper tick bound
    requiredAmountA, // u64: required Token A
    requiredAmountB, // u64: required Token B
    feeBps,          // u16: fee in basis points
    lockDuration,    // i64: lock duration in seconds
    multiplier,      // u16: reward multiplier (150 = 1.5x)
    isActive         // bool: whether level is active
  )
  .accounts({
    admin: adminPubkey,
    levelConfig: levelConfigPda,
    whirlpool: whirlpoolPubkey,
    tokenMintA: tokenMintA,
    tokenMintB: tokenMintB,      // Must match whirlpool's Token B
    layerTokenMint: tokenMintB,   // Same as Token B
  })
  .rpc();
```

#### evolve_bond
Evolves a bond from current level to the next level.

```typescript
// Step 1: Create Ed25519 signature instruction
const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
  publicKey: oraclePublicKey,
  signature: signature,
  message: evolutionMessage,
});

// Step 2: Build evolve_bond instruction
const evolveTx = await evolutionProgram.methods
  .evolveBond(
    targetLevel,     // u8: target level (2, 3, or 4)
    amountA,         // u64: Token A to deposit
    amountB,         // u64: Token B to deposit
    liquidity,       // u128: liquidity amount
    tokenMaxA,       // u64: max Token A (slippage)
    tokenMaxB,       // u64: max Token B (slippage)
    tickLower,       // i32: lower tick
    tickUpper,       // i32: upper tick
    nonce            // u64: replay protection nonce
  )
  .accounts({
    user: userPubkey,
    sourceBondMint: sourceBondMintPubkey,
    sourceCustody: sourceCustodyPda,
    targetBondMint: newBondMintKeypair.publicKey,
    userTokenAAccount: userTokenAAccount,
    userTokenBAccount: userTokenBAccount,
    whirlpool: levelConfig.whirlpool,
    positionMint: newPositionMintKeypair.publicKey,
    // ... additional accounts
  })
  .signers([newBondMintKeypair, newPositionMintKeypair])
  .rpc();
```

### Evolution Flow

```
1. VALIDATION PHASE
   ├─ Verify Ed25519 oracle signature
   ├─ Verify user owns source bond NFT
   ├─ Verify source bond level + 1 == target level
   ├─ Verify level config is active
   └─ Verify nonce is valid (replay protection)

2. BURN SOURCE BOND
   └─ token::burn(source_bond, 1)

3. TRANSFER TOKENS
   ├─ Transfer Token A from user to program
   └─ Transfer Token B from user to program

4. DEDUCT FEES
   └─ Transfer fee to treasury

5. OPEN WHIRLPOOL POSITION
   ├─ CPI to Orca Whirlpool open_position
   └─ Position owned by layer_token_authority PDA

6. ADD LIQUIDITY
   └─ CPI to Orca Whirlpool increase_liquidity

7. MINT EVOLVED BOND NFT
   ├─ Initialize new bond mint
   └─ Mint 1 NFT to user

8. UPDATE STATE
   ├─ Initialize position_custody for new bond
   ├─ Create evolution_record
   └─ Increment evolution counter

9. EMIT EVENT
   └─ emit!(BondEvolved { ... })
```

### Evolution Error Codes

| Code | Name | Description |
|------|------|-------------|
| 6000 | InvalidEvolutionLevel | Target level must be source level + 1 |
| 6001 | MaxLevelReached | Bond is already at maximum level (4) |
| 6010 | InvalidTickIndex | Tick index out of bounds |
| 6017 | TokenMaxExceeded | Slippage exceeded |
| 6022 | InvalidCustodyPda | Source custody PDA validation failed |
| 6029 | Ed25519InstructionNotFound | Missing Ed25519 verify instruction |
| 6032 | MessageReconstructionFailed | Signature message mismatch |

### Running Evolution Scripts

```bash
# Configure environment
export ANCHOR_PROVIDER_URL=https://api.devnet.solana.com
export ANCHOR_WALLET=~/.config/solana/id.json

# Run Level 1 → Level 2 evolution
npx ts-node scripts/evolve-bond-devnet.ts

# Run Level 2 → Level 3 evolution
npx ts-node scripts/target-level-3.ts

# Run Level 3 → Level 4 evolution
npx ts-node scripts/target-level-4.ts

# Fix level configuration (if token mismatch)
npx ts-node scripts/fix-level-config.ts
```

### Important Notes

1. **Token B Configuration**: Each level's `tokenMintB` and `layerTokenMint` must match the actual Token B of the configured whirlpool. Use the fix scripts if there's a mismatch.

2. **Source Custody Derivation**: 
   - For L1 bonds (from base program): derive with `LP_BONDS_PROGRAM_ID`
   - For L2+ bonds (from evolution): derive with `EVOLUTION_PROGRAM_ID`

3. **Transaction Size**: Evolution transactions are large (~35 accounts). Use Address Lookup Tables (ALT) for versioned transactions.

4. **Compute Budget**: Set compute units to 400,000+ for evolution transactions.

## License

Apache-2.0
