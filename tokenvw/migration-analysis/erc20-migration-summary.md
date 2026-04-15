# ERC20 → Solana Migration Summary

**Source:** `contracts/lp-token/LPToken.sol` (Energi Core, Solidity 0.8.22)
**Target:** `programs/lp_token/` (Anchor 0.31.1, Rust)
**Date:** 2026-03-14

---

## Architecture Comparison

| Dimension | EVM LPToken | Solana lp_token |
|-----------|-------------|-----------------|
| Runtime | EVM bytecode on Ethereum-compatible chain | BPF bytecode on Solana |
| Token accounting | ERC20 storage in contract (balanceOf mapping) | SPL Token Program (separate token accounts) |
| Token standard | ERC20Upgradeable (OpenZeppelin) | SPL Token v1 |
| Upgradeability | TransparentUpgradeableProxy + ProxyAdmin | BPFLoaderUpgradeable (native Solana) |
| Decimals | 18 | 9 (u64 constraint; see note) |
| Balance storage | `mapping(address => uint256)` in contract | Separate `TokenAccount` accounts (ATAs) |
| Allowance storage | `mapping(address => mapping(address => uint256))` | SPL `delegated_amount` on token account |
| Minter registry | `mapping(address => bool)` in contract storage | `MinterRecord` PDA per (mint, minter) |
| Pause flag | `bool` in contract storage | `bool` in `TokenState` PDA |
| Owner | `OwnableUpgradeable.owner` | `TokenState.owner` Pubkey |
| Reentrancy | `ReentrancyGuardUpgradeable` modifier | Implicit in Solana's architecture |

---

## Instruction-to-Function Mapping

| Solana Instruction | EVM Function | Notes |
|-------------------|-------------|-------|
| `initialize_mint` | `initialize(name_, symbol_, owner_, chainId_)` | Initializer pattern preserved; name/symbol stored via Metaplex |
| `mint_tokens` | `mint(address, uint256)` | Access control and pause guard preserved |
| `burn_tokens` | `burn(address, uint256)` | Access control and pause guard preserved; requires co-sign |
| `update_minter` | `updateMinter(address, bool)` | Duplicate prevention preserved |
| `set_pause(true)` | `pause()` | State guard preserved |
| `set_pause(false)` | `unpause()` | State guard preserved |
| `transfer_tokens` | `transfer(to, amount)` / `transferFrom` | No custom logic; thin SPL wrapper |
| `approve_delegate` | `approve(spender, amount)` | No custom logic; thin SPL wrapper |

---

## Key Behavioral Differences

### 1. Decimals: 18 → 9

**Reason:** Solana token balances are `u64` (max ≈ 1.8×10¹⁹). With 18 decimals, 1 full token = 10¹⁸ units, leaving maximum supply of only ~18 tokens. The Solana standard of 9 decimals allows ~18.4 billion tokens at full precision, which is practical for LP bond tokens.

**Impact:** Token amounts sent cross-chain or between systems must account for this decimal difference (multiply/divide by 10⁹).

---

### 2. Burn Authorization: Consent Required

**EVM behavior:** A registered minter can call `burn(address, amount)` on ANY address without that address's consent.

**Solana behavior:** `burn_tokens` requires two signers:
1. `authority` — the minter/owner (access control check)
2. `token_account_authority` — the owner of the token account being burned from

**Why:** Solana's SPL Token program requires the account holder (or a pre-approved delegate) to authorize burns. There is no equivalent to the EVM's unrestricted minter-burns-anyone pattern.

**Impact in practice:** The LP bond use case is unaffected. When a user redeems a bond position, they sign the transaction themselves, satisfying both requirements simultaneously. The exchange/locker program calls `burn_tokens` as a CPI, and the user's signature (on the outer transaction) satisfies the `token_account_authority` requirement.

---

### 3. Transfers and Approvals: Not Custom-Guarded (Matches EVM)

LPToken.sol does NOT override `_transfer` or `_approve`. Therefore, regular SPL transfers and delegate approvals are not blocked by pause — exactly matching the EVM behavior.

This is a deliberate preservation of original token behavior. The `transfer_tokens` and `approve_delegate` instructions in the Solana program apply no custom guards, which mirrors EVM exactly.

---

### 4. No Name/Symbol On-Chain

The base SPL Token program does not store `name` or `symbol`. For on-chain metadata, integrate the **Metaplex Token Metadata program** after initialization using `createMetadataAccountV3`. The `evm_chain_id` field in `TokenState` preserves cross-chain context.

---

### 5. Minter Registry: PDA vs Mapping

**EVM:** `mapping(address => bool) public minters` — O(1) lookup, no rent.

**Solana:** `MinterRecord` PDA per minter — requires a separate account (~42 bytes + rent). Adding a minter costs ~0.001 SOL in rent. The PDA structure provides deterministic derivation and cannot be forged.

---

### 6. No Reentrancy Guard Needed

The EVM uses `ReentrancyGuardUpgradeable` to prevent recursive calls. Solana's runtime prevents a program from being called recursively within the same transaction, and Anchor's account borrowing model prevents double-mutation. No explicit guard is needed.

---

## Security Considerations

| Property | Status | Notes |
|----------|--------|-------|
| Mint authority isolated | ✓ | token_state PDA — no external key |
| Owner-only governance | ✓ | pubkey constraint on token_state.owner |
| Minter role gating | ✓ | MinterRecord PDA + discriminator check |
| Pause-on-mint/burn | ✓ | is_paused constraint preserved |
| Account substitution prevention | ✓ | PDA seeds bind token_state to mint |
| Burn consent | ✓ Improved | Co-signer required (security improvement) |
| Upgrade security | ⚠ Needs action | Set upgrade authority to multisig or None post-deployment |
| Minter key compromise | ⚠ Same risk | A compromised minter can mint to arbitrary accounts |

---

## Deployment Steps

### Prerequisites

```bash
# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.18.0/install)"

# Install Anchor
cargo install --git https://github.com/coral-xyz/anchor avm --locked
avm install 0.31.1
avm use 0.31.1

# Install Node dependencies (in solana-token/)
pnpm install
```

### 1. Build the program

```bash
cd /path/to/solana-token
anchor build
```

After build, the program ID will be printed. Update `declare_id!()` in `programs/lp_token/src/lib.rs` and `Anchor.toml` with the generated program ID.

### 2. Generate a real program keypair

```bash
solana-keygen new -o target/deploy/lp_token-keypair.json
# Copy the pubkey and update declare_id! in lib.rs and Anchor.toml
```

### 3. Deploy to devnet

```bash
solana config set --url devnet
anchor deploy --program-name lp_token
```

### 4. Initialize the mint

Call `initialize_mint` with:
- `evm_chain_id`: the EVM chain ID (e.g., 1 for Ethereum mainnet)
- `decimals`: 9
- `owner`: the admin keypair's public key
- `token_mint`: a new keypair (will become the SPL mint address)

### 5. Register minters

For each authorized minter (Locker, Exchange), call `update_minter({ is_active: true })` from the owner.

### 6. Optional: Attach Metaplex metadata

```bash
# Using Metaplex CLI or Token Metadata Program
# Set name = "HELLO-GMI LP Bond L4", symbol = "HELLO-GMI-L4"
```

### 7. Lock upgrade authority (production)

```bash
# Transfer upgrade authority to multisig
solana program set-upgrade-authority <PROGRAM_ID> --new-upgrade-authority <MULTISIG_ADDRESS>

# OR make program immutable (irreversible)
solana program set-upgrade-authority <PROGRAM_ID> --final
```

---

## Running Tests

```bash
# In solana-token/
anchor test
# or for just the lp_token tests:
npx jest tests/lp_token.ts
```

The test suite covers:
- Initialization (owner, chainId, pause state, decimals, supply)
- Minting by owner and registered minters
- Minting rejection for unregistered callers
- Minting blocked by pause
- Burning by owner and registered minters (with co-sign)
- Burning rejection for unregistered callers
- Burning blocked by pause
- Minter add/remove/duplicate-prevention
- Only-owner enforcement for governance operations
- Pause/unpause state transitions and guards
- Regular transfers (not blocked by pause — LPToken behavior preserved)
- Delegate approval and delegated transfers
- Approvals not blocked by pause (LPToken behavior preserved)

---

## File Index

```
migration-analysis/
├── erc20-discovery.md      — Contract inventory and dependency map
├── erc20-specification.md  — Full behavioral spec of LPToken.sol
├── erc20-solana-design.md  — Solana account and instruction design
├── erc20-security.md       — EVM→Solana security model translation
└── erc20-migration-summary.md  (this file)

programs/lp_token/
├── Cargo.toml
├── Xargo.toml
└── src/
    ├── lib.rs               — Program entry point, instruction dispatch
    ├── errors.rs            — LPTokenError enum
    ├── events.rs            — Anchor events (EVM event equivalents)
    ├── state/
    │   ├── mod.rs
    │   ├── token_state.rs   — TokenState PDA (owner, is_paused, evm_chain_id)
    │   └── minter_record.rs — MinterRecord PDA (is_active, minter)
    └── instructions/
        ├── mod.rs
        ├── initialize_mint.rs   — Creates SPL mint + TokenState PDA
        ├── mint_tokens.rs       — Mints tokens (minter/owner, pause guard)
        ├── burn_tokens.rs       — Burns tokens (minter/owner + holder co-sign)
        ├── update_minter.rs     — Registers/deregisters minters (owner only)
        ├── set_pause.rs         — Pauses/unpauses (owner only)
        ├── transfer_tokens.rs   — SPL transfer wrapper (no custom guards)
        └── approve_delegate.rs  — SPL approve wrapper (no custom guards)

tests/
└── lp_token.ts             — Anchor TypeScript tests
```
