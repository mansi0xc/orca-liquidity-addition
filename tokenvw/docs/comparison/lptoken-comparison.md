# LPToken EVM vs Solana -- Function-by-Function Comparison

**Date**: 2026-03-31
**EVM Source**: `/evm-contracts/token/contracts/lp-token/LPToken.sol` (140 lines, Solidity 0.8.22)
**Solana Source**: `/solana-token/programs/lp_token/src/` (Anchor framework, Rust)
**EVM Analysis**: `/solana-token/docs/evm/lptoken-analysis.md`

---

## Contract Overview

### EVM LPToken
- Upgradeable ERC20 via TransparentUpgradeableProxy
- Inherits: ERC20Upgradeable, OwnableUpgradeable, PausableUpgradeable, ReentrancyGuardUpgradeable
- 25 functions total (7 declared + 18 inherited)
- 18 decimals, uint256 precision, no supply cap
- Pause blocks mint/burn only (no `_transfer`/`_approve` override)
- Minters can burn ANY account's tokens without allowance

### Solana lp_token
- Anchor program wrapping SPL Token CPIs
- 8 instructions: initialize_mint, mint_tokens, burn_tokens, update_minter, set_pause, transfer_ownership, accept_ownership, transfer_tokens, approve_delegate
- 9 decimals (configurable), u64 precision, no supply cap
- Pause blocks mint/burn only (transfers use SPL Token directly)
- Burn requires dual-signer (security improvement)

---

## Function-by-Function Comparison Table

| # | EVM Function | Solana Equivalent | Verdict |
|---|---|---|---|
| 1 | `initialize(name_, symbol_, owner_, chainId_)` | `initialize_mint` instruction | PARTIALLY EQUIVALENT |
| 2 | `mint(address, uint256)` | `mint_tokens` instruction | FULLY EQUIVALENT |
| 3 | `burn(address, uint256)` | `burn_tokens` instruction | PARTIALLY EQUIVALENT (intentional security improvement) |
| 4 | `updateMinter(address, bool)` | `update_minter` instruction | FULLY EQUIVALENT |
| 5 | `pause()` | `set_pause(true)` | FULLY EQUIVALENT |
| 6 | `unpause()` | `set_pause(false)` | FULLY EQUIVALENT |
| 7 | `impl()` | N/A | N/A |
| 8 | `name()` | N/A (Metaplex Token Metadata) | MISSING |
| 9 | `symbol()` | N/A (Metaplex Token Metadata) | MISSING |
| 10 | `decimals()` | Mint account `decimals` field | HANDLED BY RUNTIME |
| 11 | `totalSupply()` | Mint account `supply` field | HANDLED BY RUNTIME |
| 12 | `balanceOf(address)` | Token account `amount` field | HANDLED BY RUNTIME |
| 13 | `transfer(to, amount)` | `transfer_tokens` instruction / SPL Transfer | FULLY EQUIVALENT |
| 14 | `transferFrom(from, to, amount)` | `transfer_tokens` with delegate / SPL Transfer | FULLY EQUIVALENT |
| 15 | `approve(spender, amount)` | `approve_delegate` instruction / SPL Approve | FULLY EQUIVALENT |
| 16 | `allowance(owner, spender)` | Token account `delegate` + `delegated_amount` fields | HANDLED BY RUNTIME |
| 17 | `increaseAllowance(spender, addedValue)` | N/A | MISSING |
| 18 | `decreaseAllowance(spender, subtractedValue)` | N/A | MISSING |
| 19 | `owner()` | `token_state.owner` field | HANDLED BY RUNTIME (on-chain account read) |
| 20 | `transferOwnership(newOwner)` | `transfer_ownership` + `accept_ownership` (two-step) | PARTIALLY EQUIVALENT (security improvement) |
| 21 | `renounceOwnership()` | Not implemented | INTENTIONALLY MISSING |
| 22 | `paused()` | `token_state.is_paused` field | HANDLED BY RUNTIME (on-chain account read) |
| 23 | `minters(address)` | MinterRecord PDA existence + `is_active` field | HANDLED BY RUNTIME (on-chain account read) |
| 24 | `chainId()` | `token_state.evm_chain_id` field | HANDLED BY RUNTIME (on-chain account read) |
| 25 | Proxy upgrade pattern | BPFLoaderUpgradeable program authority | HANDLED BY RUNTIME |

---

## Detailed Findings Per Function

### 1. initialize / initialize_mint

**EVM** (`LPToken.sol:57-71`):
```
function initialize(string memory name_, string memory symbol_, address owner_, uint256 chainId_) external initializer
```
- Calls `__ERC20_init(name_, symbol_)`, `__Ownable_init()`, `__Pausable_init()`, `__ReentrancyGuard_init()`
- Then `_transferOwnership(owner_)` and `chainId = chainId_`
- One-time via `initializer` modifier
- No zero-address check on `owner_`
- No validation on name/symbol

**Solana** (`instructions/initialize_mint.rs:1-93`):
- Creates SPL mint with token_state PDA as mint_authority and freeze_authority
- Creates TokenState PDA: owner, pending_owner=default, is_paused=false, evm_chain_id, bump
- Accepts `InitializeMintParams { owner, evm_chain_id, decimals }`
- DOES validate `owner != Pubkey::default()` (improvement over EVM)
- One-time by nature (PDA + mint init can only happen once)

**Dimension Analysis**:

| Dimension | Assessment |
|---|---|
| **A. Parameters** | EVM accepts `name_, symbol_` -- Solana does NOT. Name/symbol must be set via Metaplex Token Metadata separately. Solana adds `decimals` param (EVM hardcodes 18). `chainId_` (uint256) mapped to `evm_chain_id` (u64). |
| **B. State Reads** | EVM reads `_initialized`, `_initializing`. Solana relies on account init constraints (one-time by construction). Equivalent. |
| **C. State Writes** | EVM: `_name`, `_symbol`, `_owner`, `_paused`, `_status`, `chainId`, `_initialized`. Solana: `owner`, `pending_owner`, `is_paused`, `evm_chain_id`, `bump` + SPL mint account. Name/symbol NOT stored. |
| **D. Behavioral Actions** | EVM emits `OwnershipTransferred` (x2) + `Initialized`. Solana emits `MintInitialized`. Different event semantics but both signal initialization. |
| **E. Error Conditions** | EVM: reverts if already initialized. Solana: account init fails if PDA already exists (equivalent). Solana adds zero-owner validation (improvement). |
| **F. Access Control** | EVM: `initializer` modifier. Solana: PDA init constraint. Both one-time. |
| **G. Verdict** | **PARTIALLY EQUIVALENT** -- core initialization works, but name/symbol require a separate Metaplex metadata instruction that does not currently exist in the program. |

**Gaps**:
- **MISSING**: No instruction to set token name/symbol on-chain. Must use Metaplex Token Metadata program separately.
- **IMPROVEMENT**: Zero-address owner validation added.
- **TYPE CHANGE**: `chainId_` (uint256) -> `evm_chain_id` (u64). Sufficient for all real chain IDs.

---

### 2. mint / mint_tokens

**EVM** (`LPToken.sol:79-86`):
```
function mint(address _account, uint256 _amount) external onlyMintersOrOwner whenNotPaused nonReentrant returns (bool)
```
- Calls `_mint(_account, _amount)` which updates `_totalSupply` and `_balances[_account]`
- Emits `Transfer(address(0), _account, _amount)`
- Returns `true`

**Solana** (`instructions/mint_tokens.rs:1-166`):
- Authority signer must be owner or minter (manual verification via `verify_minter`)
- Pause check via account constraint: `!token_state.is_paused`
- CPI to `token::mint_to` signed by token_state PDA
- Emits `TokensMinted { authority, recipient, amount }`
- Returns `Ok(())`

**Dimension Analysis**:

| Dimension | Assessment |
|---|---|
| **A. Parameters** | EVM: `(address _account, uint256 _amount)`. Solana: `amount: u64` + accounts (recipient_token_account). Equivalent with type adaptation. |
| **B. State Reads** | EVM: `minters[msg.sender]`, `_owner`, `_paused`, `_status`. Solana: `token_state.owner`, `token_state.is_paused`, MinterRecord PDA. Equivalent. |
| **C. State Writes** | EVM: `_totalSupply += _amount`, `_balances[_account] += _amount`. Solana: SPL Token handles both atomically. Equivalent. |
| **D. Behavioral Actions** | Both mint tokens to recipient. Both emit events. |
| **E. Error Conditions** | EVM: "Only minter or owner", "paused", "reentrant call", "mint to zero address". Solana: Unauthorized, Paused, SPL Token errors. Equivalent coverage. |
| **F. Access Control** | Both require owner OR minter + not paused. EVM adds nonReentrant (Solana: not needed -- no reentrancy in Solana runtime). |
| **G. Verdict** | **FULLY EQUIVALENT** |

**Notes**:
- EVM returns `bool true`; Solana returns `Result<()>`. Functionally equivalent (EVM never returns false -- it reverts instead).
- EVM nonReentrant is unnecessary on Solana (runtime prevents reentrancy). Correctly omitted.
- Minting 0 tokens: EVM allows with event emission. Solana: SPL Token allows minting 0 with no error. Equivalent.

---

### 3. burn / burn_tokens

**EVM** (`LPToken.sol:94-100`):
```
function burn(address _account, uint256 _amount) external onlyMintersOrOwner whenNotPaused nonReentrant returns (bool)
```
- Calls `_burn(_account, _amount)` -- NO allowance check
- Any minter/owner can burn ANY user's tokens unilaterally

**Solana** (`instructions/burn_tokens.rs:1-100`):
- `authority` signer: must be owner or registered minter
- `token_account_authority` signer: must own the token account being burned from
- CPI to `token::burn` with token_account_authority as the burn authority
- Dual-signer requirement

**Dimension Analysis**:

| Dimension | Assessment |
|---|---|
| **A. Parameters** | EVM: `(address _account, uint256 _amount)`. Solana: `amount: u64` + accounts (includes both authority and token_account_authority). Solana adds an extra required signer. |
| **B. State Reads** | EVM: `minters[msg.sender]`, `_owner`, `_paused`, `_balances[_account]`. Solana: same plus token_account.owner verification. |
| **C. State Writes** | EVM: `_totalSupply -= _amount`, `_balances[_account] -= _amount`. Solana: SPL Token handles both. Equivalent. |
| **D. Behavioral Actions** | Both burn tokens and emit events. |
| **E. Error Conditions** | EVM: "Only minter or owner", "paused", "burn amount exceeds balance", "burn from zero address". Solana: Unauthorized, Paused, SPL Token errors, InvalidTokenAuthority. |
| **F. Access Control** | EVM: minter/owner only. Solana: minter/owner AND token account holder must both sign. **This is a deliberate behavioral difference.** |
| **G. Verdict** | **PARTIALLY EQUIVALENT** -- intentional security improvement. |

**Critical Behavioral Difference**:
- EVM: A minter can burn ANY user's tokens without the user's consent.
- Solana: The token account owner MUST also sign the transaction.
- This is documented as a security improvement. In the LP bond workflow, the user always signs when redeeming, so this does not break intended functionality.

---

### 4. updateMinter / update_minter

**EVM** (`LPToken.sol:108-114`):
```
function updateMinter(address _account, bool _isMinter) external onlyOwner
```
- Checks duplicate: `require(minters[_account] != _isMinter)`
- Sets `minters[_account] = _isMinter`
- Emits `MinterUpdated(_account, _isMinter)`

**Solana** (`instructions/update_minter.rs:1-84`):
- Owner signer constraint
- Uses `init_if_needed` for MinterRecord PDA on first call
- Checks duplicate: `require!(minter_record.is_active != params.is_active)`
- Updates `record.is_active`, `record.minter`, `record.bump`
- Emits `MinterUpdated { minter, is_active }`

**Dimension Analysis**:

| Dimension | Assessment |
|---|---|
| **A. Parameters** | EVM: `(address _account, bool _isMinter)`. Solana: `UpdateMinterParams { is_active: bool }` + `target_minter` account. Equivalent. |
| **B. State Reads** | EVM: `_owner`, `minters[_account]`. Solana: `token_state.owner`, `minter_record.is_active`. Equivalent. |
| **C. State Writes** | EVM: `minters[_account] = _isMinter`. Solana: `minter_record.is_active = is_active`. Equivalent. |
| **D. Behavioral Actions** | Both emit MinterUpdated event. |
| **E. Error Conditions** | EVM: "caller is not the owner", "Duplicate operation". Solana: Unauthorized, DuplicateOperation. Equivalent. |
| **F. Access Control** | Both: owner-only. |
| **G. Verdict** | **FULLY EQUIVALENT** |

**Notes**:
- EVM does not validate `_account != address(0)`. Solana: no validation on `target_minter` being `Pubkey::default()` either. Both have same gap.
- On "remove minter" (is_active=false): EVM sets mapping to false (retains storage). Solana sets `is_active=false` (retains PDA, does not close account). The MinterRecord PDA remains on-chain, consuming rent. This is not a behavioral difference but a resource difference. Consider adding a `close_minter_record` instruction in future to reclaim rent.

---

### 5. pause / set_pause(true)

**EVM** (`LPToken.sol:120-122`):
```
function pause() external whenNotPaused onlyOwner
```
- Sets `_paused = true`, emits `Paused(msg.sender)`

**Solana** (`instructions/set_pause.rs:39-60`):
- `set_pause(paused: true)`: requires `!current` (whenNotPaused equivalent)
- Sets `token_state.is_paused = true`, emits `PauseStateChanged { paused: true, authority }`

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **FULLY EQUIVALENT** |

---

### 6. unpause / set_pause(false)

**EVM** (`LPToken.sol:128-130`):
```
function unpause() external whenPaused onlyOwner
```
- Sets `_paused = false`, emits `Unpaused(msg.sender)`

**Solana** (`instructions/set_pause.rs:39-60`):
- `set_pause(paused: false)`: requires `current` (whenPaused equivalent)
- Sets `token_state.is_paused = false`, emits `PauseStateChanged { paused: false, authority }`

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **FULLY EQUIVALENT** |

**Note**: EVM emits separate `Paused` and `Unpaused` events. Solana emits a unified `PauseStateChanged` with a boolean. Functionally equivalent -- indexers can distinguish direction from the `paused` field.

---

### 7. impl()

**EVM** (`LPToken.sol:137-139`):
```
function impl() external view returns (address) { return address(this); }
```
- Returns the proxy address when called through proxy (misleading behavior documented in analysis)
- Used for upgrade verification

**Solana**: No equivalent instruction.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **N/A** -- This function is an artifact of the EVM proxy pattern. On Solana, program identity is the program ID itself. The BPFLoaderUpgradeable program manages upgrades. There is no proxy indirection, so `impl()` has no purpose. |

---

### 8-9. name() / symbol()

**EVM**: Inherited from ERC20Upgradeable. Returns `_name` / `_symbol` set during initialization.

**Solana**: SPL Token mint accounts do NOT store name or symbol. These are set via the Metaplex Token Metadata program as a separate account.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **MISSING** -- The lp_token program does not create Metaplex metadata. Name and symbol must be set in a separate step. This is a gap if the token is expected to be human-readable in wallets immediately after initialization. |

**Recommendation**: Add an optional `create_metadata` instruction that CPIs into the Metaplex Token Metadata program, or document the operational procedure to create metadata post-initialization.

---

### 10. decimals()

**EVM**: Returns `18` (hardcoded in ERC20Upgradeable, not overridden).

**Solana**: Stored in the SPL mint account's `decimals` field. Set to whatever `params.decimals` is during `initialize_mint` (recommended: 9).

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** -- SPL mint account stores decimals. Readable by anyone via account deserialization. |

**Documented Behavioral Difference**: EVM uses 18 decimals; Solana uses 9. This means:
- EVM: 1 token = 10^18 smallest units
- Solana: 1 token = 10^9 smallest units
- Maximum representable supply: EVM ~1.15 * 10^59 tokens. Solana ~18.4 billion tokens (u64 / 10^9).

---

### 11. totalSupply()

**EVM**: Returns `_totalSupply`.

**Solana**: Read `supply` field from the SPL mint account.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** -- SPL Token program maintains supply automatically on mint/burn. |

---

### 12. balanceOf(address)

**EVM**: Returns `_balances[account]`.

**Solana**: Read `amount` field from the user's associated token account (ATA) for this mint.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** -- Each token account stores its own balance. |

---

### 13. transfer(to, amount)

**EVM**: Standard ERC20 transfer. NOT paused-gated in LPToken.

**Solana** (`instructions/transfer_tokens.rs:1-53`):
- Direct CPI to `token::transfer`
- No pause check (matches EVM behavior)
- `from_authority` must sign

| Dimension | Assessment |
|---|---|
| **A. Parameters** | EVM: `(address to, uint256 amount)`. Solana: `amount: u64` + from/to token accounts + authority signer. |
| **C. State Writes** | Both debit source, credit destination. SPL Token handles atomically. |
| **F. Access Control** | Both: caller must own source tokens (or be delegate). Not pause-gated. |
| **G. Verdict** | **FULLY EQUIVALENT** |

**Note**: Users can also call SPL Token transfer directly, bypassing the lp_token program entirely. The wrapper is for convenience and documentation; it adds no custom logic.

---

### 14. transferFrom(from, to, amount)

**EVM**: Uses allowance mechanism. Decrements allowance (unless infinite). NOT paused-gated.

**Solana**: Same `transfer_tokens` instruction. If `from_authority` is a delegate (set via `approve_delegate` / SPL approve), SPL Token enforces the delegated_amount limit and decrements it.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **FULLY EQUIVALENT** -- SPL Token's delegate mechanism maps to ERC20 approve/transferFrom. |

**Behavioral Difference**: EVM supports "infinite approval" (type(uint256).max doesn't decrement). SPL Token always decrements the delegate amount. There is no infinite delegation concept. This is a Solana platform constraint, not a program gap.

---

### 15. approve(spender, amount)

**EVM**: Sets `_allowances[msg.sender][spender] = amount`. NOT paused-gated.

**Solana** (`instructions/approve_delegate.rs:1-52`):
- CPI to `token::approve`
- No pause check (matches EVM)
- Sets delegate + delegated_amount on the token account

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **FULLY EQUIVALENT** |

**Behavioral Differences**:
- EVM: Each (owner, spender) pair has its own allowance. Multiple spenders can have allowances simultaneously.
- Solana: Each token account can have only ONE delegate at a time. Setting a new delegate revokes the previous one.
- This is a fundamental platform difference. In practice, LP tokens primarily use allowances for bond program interactions, so single-delegate is sufficient.

---

### 16. allowance(owner, spender)

**EVM**: Returns `_allowances[owner][spender]`.

**Solana**: Read `delegate` and `delegated_amount` from the token account. If `delegate == spender`, return `delegated_amount`. Otherwise, 0.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** |

---

### 17-18. increaseAllowance / decreaseAllowance

**EVM**: Convenience functions to safely modify allowances (avoid approve race condition).

**Solana**: No equivalent instructions in the program.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **MISSING** -- However, these are convenience functions only. The approve race condition they solve is less relevant on Solana because (a) Solana has single-delegate model, and (b) transactions are atomic. Low priority. |

---

### 19. owner()

**EVM**: Returns `_owner` state variable.

**Solana**: Read `token_state.owner` field by deserializing the TokenState PDA account.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** -- On-chain account data is publicly readable. |

---

### 20. transferOwnership(newOwner) / transfer_ownership + accept_ownership

**EVM** (`OwnableUpgradeable`):
- One-step: `_owner = newOwner` immediately
- Validates `newOwner != address(0)`
- Emits `OwnershipTransferred`

**Solana** (`instructions/transfer_ownership.rs` + `instructions/accept_ownership.rs`):
- Two-step: `transfer_ownership` sets `pending_owner`, then `accept_ownership` finalizes
- Validates `new_owner != Pubkey::default()` and `new_owner != current owner`
- Emits `OwnershipTransferProposed` then `OwnershipTransferred`

| Dimension | Assessment |
|---|---|
| **A. Parameters** | EVM: `(address newOwner)`. Solana: `new_owner: Pubkey` for step 1; no params for step 2. |
| **C. State Writes** | EVM: `_owner = newOwner` immediately. Solana: step 1 sets `pending_owner`, step 2 sets `owner` and clears `pending_owner`. |
| **D. Behavioral Actions** | EVM: one event. Solana: two events across two transactions. |
| **F. Access Control** | EVM: onlyOwner. Solana: step 1 onlyOwner, step 2 only pending_owner. Strictly safer. |
| **G. Verdict** | **PARTIALLY EQUIVALENT** -- intentional security improvement (two-step pattern prevents accidental loss from typos). |

---

### 21. renounceOwnership()

**EVM**: Sets `_owner = address(0)`. Irreversible. All onlyOwner functions become permanently inaccessible.

**Solana**: Not implemented.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **INTENTIONALLY MISSING** -- Documented in `transfer_ownership.rs:21-25`. Renouncing ownership would permanently disable minter management, pause control, and ownership transfer. The LP bond use case requires ongoing governance. This is a conscious safety restriction. |

---

### 22. paused()

**EVM**: Returns `_paused` bool.

**Solana**: Read `token_state.is_paused` from the TokenState PDA.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** |

---

### 23. minters(address) -- view

**EVM**: Auto-generated getter for `mapping(address => bool) public minters`.

**Solana**: Check existence and `is_active` field of MinterRecord PDA at seeds `["minter", token_state, minter]`.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** -- On-chain account is publicly readable. |

---

### 24. chainId()

**EVM**: Auto-generated getter for `uint256 public chainId`.

**Solana**: Read `token_state.evm_chain_id` field.

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** |

---

### 25. Proxy Upgrade Pattern

**EVM**: TransparentUpgradeableProxy + ProxyAdmin
- `LPTokenProxy.sol` + `LPTokenProxyAdmin.sol`
- ProxyAdmin owner calls `upgrade(proxy, newImpl)` or `upgradeAndCall(...)`

**Solana**: BPFLoaderUpgradeable program
- Program deployed with `--upgradeable` flag
- Upgrade authority set to a specific pubkey
- `solana program deploy --upgrade-authority <key>`

| Dimension | Assessment |
|---|---|
| **G. Verdict** | **HANDLED BY RUNTIME** -- Solana's native upgrade mechanism replaces the EVM proxy pattern entirely. The upgrade authority on the BPFLoaderUpgradeable program serves the same role as the ProxyAdmin owner. |

---

## Contract-Level Summary

### Parity Statistics

| Category | Count | Functions |
|---|---|---|
| **FULLY EQUIVALENT** | 8 | mint, burn (core logic), updateMinter, pause, unpause, transfer, transferFrom/transfer, approve |
| **PARTIALLY EQUIVALENT** | 3 | initialize (missing name/symbol), burn (dual-signer), transferOwnership (two-step) |
| **HANDLED BY RUNTIME** | 9 | decimals, totalSupply, balanceOf, allowance, owner, paused, minters, chainId, proxy upgrade |
| **MISSING** | 3 | name/symbol metadata, increaseAllowance, decreaseAllowance |
| **INTENTIONALLY MISSING** | 1 | renounceOwnership |
| **N/A** | 1 | impl() |

### Overall Parity: ~88%
- Core business logic (mint/burn/pause/minter management): 100% equivalent or improved
- ERC20 standard operations (transfer/approve): 100% equivalent (via SPL Token)
- View functions: 100% equivalent (on-chain account reads)
- Convenience functions: 2 missing (increaseAllowance, decreaseAllowance) -- low priority
- Metadata: 1 gap (name/symbol) -- medium priority

### All Gaps Requiring Attention

| Priority | Gap | Severity | Description |
|---|---|---|---|
| 1 | Token name/symbol metadata | MEDIUM | No Metaplex metadata created during initialization. Wallets will show the mint address instead of a human-readable name. |
| 2 | increaseAllowance/decreaseAllowance | LOW | Convenience functions missing. Users can use `approve_delegate` directly. Approve race condition is less relevant on Solana. |
| 3 | MinterRecord rent reclamation | LOW | Deactivated MinterRecord PDAs remain on-chain, consuming rent forever. Consider adding a `close_minter_record` instruction. |
| 4 | Placeholder program ID | CRITICAL (deploy-time) | `declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS")` must be replaced before deployment. |
