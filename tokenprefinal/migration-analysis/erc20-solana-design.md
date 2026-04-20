# Solana Program Design — LP Token

## Overview

The ERC20 LPToken is translated to a Solana Anchor program (`lp_token`) that:
- Uses the **SPL Token Program** for all balance accounting (no reimplementation)
- Wraps SPL calls with the same access control semantics as the EVM contract
- Preserves: minter roles, pause-on-mint/burn, owner-only governance
- Differences from EVM are explicitly documented

---

## Concept Mapping — ERC20 → Solana

| ERC20 Concept | Solana Equivalent |
|---------------|-------------------|
| `ERC20.balanceOf` | SPL token account balance |
| `ERC20.totalSupply` | SPL mint's `supply` field |
| `ERC20.decimals` (18) | SPL mint `decimals` field (9, see note) |
| `ERC20.transfer` | SPL `token::transfer` CPI |
| `ERC20.approve` | SPL `token::approve` CPI |
| `ERC20.transferFrom` | SPL `token::transfer` with delegate authority |
| `ERC20.allowance` | SPL token account `delegated_amount` |
| ERC20 `maxSupply` | None — LPToken has no max supply; SPL mint has no cap |
| `minters[addr]` | `MinterRecord` PDA per (mint, minter address) |
| `owner` | `TokenState.owner` pubkey |
| `paused` | `TokenState.is_paused` bool |
| `chainId` | `TokenState.evm_chain_id` u64 |
| `_mint()` | `token::mint_to` CPI (signed by `token_state` PDA) |
| `_burn()` | `token::burn` CPI (signed by token account owner) |
| `msg.sender` check | `Signer<'info>` constraint |
| `onlyOwner` modifier | `constraint = authority.key() == token_state.owner` |
| `onlyMintersOrOwner` | runtime check: owner OR `MinterRecord.is_active == true` |
| `whenNotPaused` | `constraint = !token_state.is_paused` |
| `nonReentrant` | Anchor's single-transaction model (Solana is inherently re-entrant safe) |
| `TransparentUpgradeableProxy` | Solana native program upgrades via `BPFLoaderUpgradeable` |

> **Decimals Note:** SPL token accounts use `u64` for balances. With 18 decimals, 1 token = 10^18 units, and `u64::MAX` ≈ 18.4 × 10^18, meaning the max token supply would be only ~18 tokens. This is impractical. The Solana program uses **9 decimals**, which is standard for Solana SPL tokens and allows up to ~18.4 billion tokens with full precision.

---

## Program Accounts

### `TokenState` — Program Control Account (PDA)

**Seeds:** `[b"token_state", mint_pubkey.as_ref()]`

| Field | Type | Description |
|-------|------|-------------|
| `owner` | `Pubkey` | Admin authority (maps to EVM `owner`) |
| `is_paused` | `bool` | Pause flag blocking mint/burn |
| `evm_chain_id` | `u64` | EVM chain ID this token corresponds to |
| `bump` | `u8` | PDA bump seed for signing |

**INIT_SPACE:** 32 + 1 + 8 + 1 = 42 bytes (+8 discriminator = 50 total)

**Purpose:**
- Holds all governance state
- Acts as the **mint authority** on the SPL mint (allowing the program to CPI into `mint_to`)
- Acts as the **freeze authority** on the SPL mint (for potential future freeze features)
- Its PDA seeds allow it to sign CPI calls via `seeds` signer

---

### `MinterRecord` — Per-Minter Authorization Account (PDA)

**Seeds:** `[b"minter", token_state_pubkey.as_ref(), minter_pubkey.as_ref()]`

| Field | Type | Description |
|-------|------|-------------|
| `is_active` | `bool` | Whether this minter is authorized |
| `minter` | `Pubkey` | The minter's public key (for reference) |
| `bump` | `u8` | PDA bump seed |

**INIT_SPACE:** 1 + 32 + 1 = 34 bytes (+8 discriminator = 42 total)

**Purpose:**
- Maps to `mapping(address => bool) public minters` in EVM
- Created/updated via `update_minter` (owner only)
- Checked at runtime in `mint_tokens` and `burn_tokens`

---

### SPL Mint Account

Standard SPL token mint, initialized by `initialize_mint`.

| SPL Field | Value |
|-----------|-------|
| `mint_authority` | `token_state` PDA |
| `freeze_authority` | `token_state` PDA |
| `decimals` | 9 (configurable at init) |
| `supply` | Dynamic, increases with `mint_to`, decreases with `burn` |

---

### SPL Token Accounts (user-owned)

Standard Associated Token Accounts (ATAs) owned by end users.
Not managed by the `lp_token` program directly — created via `spl-associated-token-account`.

---

## Authority Model

```
Owner (keypair)
    │
    ├── Holds token_state.owner
    ├── Can: update_minter, set_pause, transfer_ownership
    │
    └── TokenState PDA  [seeds: "token_state" + mint]
            │
            ├── Holds mint_authority on SPL mint
            ├── Holds freeze_authority on SPL mint
            └── Signs CPI calls for mint_to

MinterRecord PDA  [seeds: "minter" + token_state + minter_pubkey]
    │
    └── is_active == true  →  that pubkey may call mint_tokens / burn_tokens

SPL Token Program
    │
    ├── Manages all balances in token accounts
    ├── Enforces transfer / approve / transferFrom semantics
    └── Performs mint_to (requires mint_authority signature = token_state PDA)
         and burn (requires token account owner signature)
```

---

## Instruction List

### 1. `initialize_mint`

**EVM equivalent:** `initialize()` constructor

**Accounts:**
- `payer` (mut, signer) — funds account creation
- `token_mint` (mut, signer) — fresh keypair for the new SPL mint
- `token_state` (init, PDA) — program control account
- `token_program` — SPL Token Program
- `system_program` — Solana System Program
- `rent` — Rent sysvar

**Params:**
- `evm_chain_id: u64` — EVM chain ID
- `decimals: u8` — token decimals (typically 9)
- `owner: Pubkey` — initial admin

**Actions:**
1. Create SPL mint with `token_state` PDA as mint_authority and freeze_authority
2. Create `TokenState` PDA with provided owner and evm_chain_id
3. Set `is_paused = false`

---

### 2. `mint_tokens`

**EVM equivalent:** `mint(address _account, uint256 _amount)`

**Accounts:**
- `authority` (signer) — must be owner or active minter
- `token_state` (PDA, constraint: not paused)
- `minter_record` (unchecked) — MinterRecord PDA, verified in handler
- `token_mint` (mut) — the SPL mint
- `recipient_token_account` (mut) — destination SPL token account
- `token_program` — SPL Token Program

**Params:**
- `amount: u64`

**Actions:**
1. Assert `!token_state.is_paused`
2. Assert `authority == token_state.owner` OR `minter_record.is_active == true`
3. CPI `token::mint_to` signed by `token_state` PDA seeds

---

### 3. `burn_tokens`

**EVM equivalent:** `burn(address _account, uint256 _amount)`

**Accounts:**
- `authority` (signer) — must be owner or active minter
- `token_account_authority` (signer) — owner of the token account being burned from
- `token_state` (PDA, constraint: not paused)
- `minter_record` (unchecked) — MinterRecord PDA, verified in handler
- `token_mint` (mut) — the SPL mint
- `token_account` (mut) — source SPL token account
- `token_program` — SPL Token Program

**Params:**
- `amount: u64`

**Actions:**
1. Assert `!token_state.is_paused`
2. Assert `authority == token_state.owner` OR `minter_record.is_active == true`
3. Assert `token_account.owner == token_account_authority.key()`
4. CPI `token::burn` signed by `token_account_authority`

> **Behavioral difference from EVM:** On Solana, burning requires the token account holder to co-sign. EVM allowed minters to burn from any address without consent. The dual-signer model is a security improvement.

---

### 4. `update_minter`

**EVM equivalent:** `updateMinter(address _account, bool _isMinter)`

**Accounts:**
- `owner` (signer) — must equal `token_state.owner`
- `token_state` (PDA)
- `minter_record` (init_if_needed, PDA) — seeds: `[b"minter", token_state, target_minter]`
- `target_minter` — the address being registered/deregistered
- `system_program`

**Params:**
- `is_active: bool`

**Actions:**
1. Assert `owner.key() == token_state.owner`
2. Assert `minter_record.is_active != is_active` (duplicate prevention)
3. Set `minter_record.is_active = is_active`
4. Set `minter_record.minter = target_minter.key()`
5. Emit `MinterUpdated` event

---

### 5. `set_pause`

**EVM equivalent:** `pause()` / `unpause()`

**Accounts:**
- `owner` (signer) — must equal `token_state.owner`
- `token_state` (mut, PDA)

**Params:**
- `paused: bool`

**Actions:**
1. Assert `owner.key() == token_state.owner`
2. If `paused == true`: assert `!token_state.is_paused` (matches `whenNotPaused`)
3. If `paused == false`: assert `token_state.is_paused` (matches `whenPaused`)
4. Set `token_state.is_paused = paused`
5. Emit `PauseStateChanged` event

---

### 6. `transfer_tokens`

**EVM equivalent:** `transfer(address to, uint256 amount)` (standard ERC20, no custom logic)

**Accounts:**
- `from_authority` (signer) — owner of source token account
- `from_token_account` (mut)
- `to_token_account` (mut)
- `token_program`

**Params:**
- `amount: u64`

**Actions:**
1. CPI `token::transfer` (no custom guards — LPToken does not override `_transfer`)

> This instruction is a thin wrapper for documentation completeness. Users may also call SPL Token directly.

---

### 7. `approve_delegate`

**EVM equivalent:** `approve(address spender, uint256 amount)` (standard ERC20, no custom logic)

**Accounts:**
- `token_account_owner` (signer)
- `token_account` (mut)
- `delegate`
- `token_program`

**Params:**
- `amount: u64`

**Actions:**
1. CPI `token::approve` (no custom guards — LPToken does not override `_approve`)

> This instruction is a thin wrapper for documentation completeness. Users may also call SPL Token directly.

---

## Events (Anchor)

| Anchor Event | EVM Event |
|--------------|-----------|
| `MinterUpdated { minter, is_active }` | `MinterUpdated(address indexed, bool)` |
| `PauseStateChanged { paused }` | `Paused(address)` / `Unpaused(address)` |

> SPL Transfer and Approval events are emitted by the SPL Token Program internally.

---

## Behavioral Differences Summary

| Behavior | EVM LPToken | Solana lp_token |
|----------|-------------|-----------------|
| Decimals | 18 | 9 (u64 constraint) |
| Burn authorization | Minter can burn from ANY address | Minter + token account owner must both sign |
| Pause effect on transfer | Not blocked | Not blocked (matches EVM) |
| Pause effect on mint/burn | Blocked | Blocked (matches EVM) |
| Upgradeability | TransparentProxy | BPFLoaderUpgradeable |
| Name/Symbol on-chain | ERC20 metadata stored in contract | SPL mint has no name/symbol field (use Metaplex for metadata) |
| Minter registry | `mapping(address => bool)` | `MinterRecord` PDA per minter |
| Reentrancy protection | `nonReentrant` modifier | Solana's architecture prevents reentrancy by design |
