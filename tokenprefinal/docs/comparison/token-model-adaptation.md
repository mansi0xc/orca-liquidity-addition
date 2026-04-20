# Token Model Adaptation: ERC20 to SPL Token

**Date**: 2026-03-31

---

## Fundamental Model Differences

### ERC20 (EVM)
- Single contract holds ALL state: balances, allowances, supply, metadata
- `mapping(address => uint256)` for balances
- `mapping(address => mapping(address => uint256))` for allowances
- msg.sender pattern for authorization
- All operations are function calls on the same contract

### SPL Token (Solana)
- Separate accounts: mint account (defines token), token accounts (hold balances)
- Each user has one Associated Token Account (ATA) per mint
- Authority model: owner or delegate can operate on a token account
- Mint authority (stored on mint account) controls minting
- CPI (Cross-Program Invocation) pattern for token operations

---

## How ERC20 Concepts Were Adapted

### 1. Balance Storage

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `_balances[address]` mapping | Each user's ATA holds their balance in its `amount` field |
| Single source of truth | Distributed across individual token accounts |
| Direct mapping read | Account deserialization required |

**How it works**: When a user creates an ATA for the LP token mint, that ATA stores their balance. There is no central mapping. The `balanceOf` equivalent is reading the `amount` field of the user's token account.

### 2. Supply Tracking

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `_totalSupply` variable | Mint account `supply` field |
| Manually incremented in `_mint` | Automatically updated by SPL Token program on mint_to/burn |
| Direct read | Account deserialization of mint account |

**No custom code needed**: The SPL Token program atomically updates `supply` whenever `mint_to` or `burn` is called via CPI.

### 3. Allowance / Approval

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `_allowances[owner][spender]` nested mapping | Token account `delegate` + `delegated_amount` fields |
| Multiple spenders per owner | **Only ONE delegate per token account** |
| Infinite approval (type(uint256).max) | No infinite delegation concept |
| `approve` + `transferFrom` | `token::approve` + `token::transfer` (with delegate as authority) |

**Key difference**: ERC20 allows unlimited concurrent allowances. SPL Token allows exactly one delegate with one amount per token account. Setting a new delegate implicitly revokes the previous one. For the LP bond use case, this is sufficient since only one program (the bond program) needs delegation at a time.

### 4. Mint Authority

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `onlyMintersOrOwner` modifier on `mint()` | token_state PDA is the mint_authority on the SPL mint |
| Any authorized caller invokes `_mint()` | lp_token program CPIs into SPL Token `mint_to`, signing with PDA seeds |
| Contract itself mints | Program signs CPI as the mint authority |

**How it works**: The TokenState PDA (seeds: `["token_state", mint_pubkey]`) is set as the `mint_authority` on the SPL mint during initialization. When `mint_tokens` is called, the program verifies access control (owner or active minter), then performs a CPI to `token::mint_to` using PDA signer seeds. The SPL Token program verifies the PDA signature matches the mint's `mint_authority`.

### 5. Burn Authority

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| Minter calls `_burn(account, amount)` directly | CPI to `token::burn` with token account owner as authority |
| No allowance check | Token account owner MUST sign (Solana security improvement) |
| Minter has unilateral burn power | Dual-signer: minter/owner + token holder |

**Security improvement**: The EVM burn pattern is a centralization risk (minters can drain any account). The Solana adaptation requires the token account owner to co-sign, preventing unauthorized burns.

### 6. Ownership and Governance

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `_owner` state variable (OwnableUpgradeable) | `token_state.owner` field in TokenState PDA |
| `onlyOwner` modifier | Anchor constraint: `owner.key() == token_state.owner` |
| One-step `transferOwnership` | Two-step `transfer_ownership` + `accept_ownership` |
| `renounceOwnership` | Intentionally not implemented |

### 7. Pause Mechanism

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `_paused` bool (PausableUpgradeable) | `token_state.is_paused` field |
| `whenNotPaused` modifier | Anchor constraint: `!token_state.is_paused` on mint/burn accounts |
| `whenPaused` modifier | `require!(current, LPTokenError::InvalidPauseState)` in set_pause |

**Scope preserved**: Pause only blocks mint/burn. Regular SPL transfers are not checked against the pause flag (users can call SPL Token transfer directly, bypassing the program entirely).

### 8. Minter Registry

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `mapping(address => bool) minters` | MinterRecord PDA per (token_state, minter) pair |
| Simple mapping read | PDA derivation + deserialization + `is_active` check |
| Storage: one slot per minter | Storage: one account (73 bytes + 8 discriminator) per minter |

**Trade-off**: The PDA approach uses more storage per minter but provides stronger type safety and prevents cross-program spoofing via program ownership checks.

### 9. Reentrancy Protection

| ERC20 Concept | SPL Token Adaptation |
|---|---|
| `ReentrancyGuardUpgradeable` | Not needed |
| `_status` variable toggled on entry/exit | Solana runtime prevents reentrancy by design |
| `nonReentrant` modifier | No equivalent needed |

**Why not needed**: Solana's runtime model processes instructions sequentially within a transaction. A program cannot be re-entered during CPI execution because the runtime locks the calling program's accounts. The reentrancy guard is correctly omitted.

### 10. Events / Logging

| ERC20 Event | Solana Event | Notes |
|---|---|---|
| `Transfer(from, to, amount)` on mint | `TokensMinted { authority, recipient, amount }` | Different shape; logs minting authority |
| `Transfer(from, address(0), amount)` on burn | `TokensBurned { authority, from, amount }` | Different shape; logs burn authority |
| `MinterUpdated(account, isMinter)` | `MinterUpdated { minter, is_active }` | Equivalent |
| `Paused(msg.sender)` | `PauseStateChanged { paused: true, authority }` | Unified event |
| `Unpaused(msg.sender)` | `PauseStateChanged { paused: false, authority }` | Unified event |
| `OwnershipTransferred(old, new)` | `OwnershipTransferProposed` + `OwnershipTransferred` | Two events for two-step transfer |
| `Initialized(version)` | `MintInitialized { mint, owner, evm_chain_id, decimals }` | Richer event data |
| `Approval(owner, spender, amount)` | SPL Token program logs | Not emitted by lp_token program |
| `Transfer(from, to, amount)` on transfer | SPL Token program logs | Not emitted by lp_token program |

---

## What Is Handled by Runtime vs Custom Code

### Handled by SPL Token Runtime (no custom code needed)
1. Balance accounting (per-account amounts)
2. Supply tracking (mint account supply field)
3. Transfer execution (debit/credit atomically)
4. Allowance/delegation (single delegate per account)
5. Overflow protection (u64 arithmetic in SPL Token)
6. Zero-balance burn prevention (SPL Token rejects if insufficient)
7. Mint authority verification (SPL Token verifies signer matches mint_authority)

### Custom Code in lp_token Program
1. Access control: owner/minter verification for mint and burn
2. Pause state management and enforcement
3. Minter registry (MinterRecord PDAs)
4. Ownership transfer (two-step)
5. Event emission for governance actions

### Handled by Solana Runtime (no equivalent needed)
1. Reentrancy protection (runtime-level)
2. Upgrade mechanism (BPFLoaderUpgradeable)
3. Account existence/initialization checks (Anchor constraints)

---

## What Is Missing from the Adaptation

| Missing Element | Impact | Recommendation |
|---|---|---|
| Token name/symbol on-chain | Wallets show mint address instead of name | Add Metaplex Token Metadata CPI or document manual procedure |
| increaseAllowance/decreaseAllowance | No safe allowance modification helpers | Low priority; single-delegate model reduces need |
| Infinite approval | Cannot set non-decrementing delegation | Platform limitation; workaround is re-approving as needed |
| Multiple concurrent allowances | Only one delegate per token account | Platform limitation; sufficient for LP bond use case |
| MinterRecord account closure | Deactivated minters consume rent forever | Add close instruction for rent reclamation |
