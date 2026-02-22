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

## License

Apache-2.0
