# EVM-to-Solana Migration Comparison: Summary Report

**Date**: 2026-03-31
**Scope**: LPToken (EVM) -> lp_token (Solana/Anchor)
**EVM Source**: `/evm-contracts/token/contracts/lp-token/LPToken.sol`
**Solana Source**: `/solana-token/programs/lp_token/src/`

---

## Overall Parity: 88%

| Category | Count | Percentage |
|---|---|---|
| FULLY EQUIVALENT | 8 | 32% |
| PARTIALLY EQUIVALENT (improved) | 3 | 12% |
| HANDLED BY RUNTIME | 9 | 36% |
| MISSING | 3 | 12% |
| INTENTIONALLY MISSING | 1 | 4% |
| N/A (EVM-specific) | 1 | 4% |
| **Total functions analyzed** | **25** | **100%** |

If we exclude N/A and HANDLED BY RUNTIME (which are fully functional), the effective parity of custom logic is:

| Custom Logic Parity | Status |
|---|---|
| Fully equivalent | 8 / 12 (67%) |
| Improved over EVM | 3 / 12 (25%) |
| Missing (non-critical) | 3 / 12 (low-severity gaps) |

---

## Master List: FULLY EQUIVALENT Functions

| # | EVM Function | Solana Instruction | File |
|---|---|---|---|
| 1 | `mint(address, uint256)` | `mint_tokens` | `instructions/mint_tokens.rs` |
| 2 | `updateMinter(address, bool)` | `update_minter` | `instructions/update_minter.rs` |
| 3 | `pause()` | `set_pause(true)` | `instructions/set_pause.rs` |
| 4 | `unpause()` | `set_pause(false)` | `instructions/set_pause.rs` |
| 5 | `transfer(to, amount)` | `transfer_tokens` | `instructions/transfer_tokens.rs` |
| 6 | `transferFrom(from, to, amount)` | `transfer_tokens` (with delegate) | `instructions/transfer_tokens.rs` |
| 7 | `approve(spender, amount)` | `approve_delegate` | `instructions/approve_delegate.rs` |
| 8 | ERC20 view functions (7) | SPL account reads | N/A (runtime) |

---

## Master List: PARTIALLY EQUIVALENT Functions (with Gaps)

| # | EVM Function | Solana Instruction | Gap Description | Nature |
|---|---|---|---|---|
| 1 | `initialize(name_, symbol_, owner_, chainId_)` | `initialize_mint` | Name and symbol parameters not stored; no Metaplex metadata created | Missing feature |
| 2 | `burn(address, uint256)` | `burn_tokens` | Requires token account owner to co-sign (EVM allows minter-only burn) | Intentional security improvement |
| 3 | `transferOwnership(newOwner)` | `transfer_ownership` + `accept_ownership` | Two-step instead of one-step (EVM is immediate) | Intentional security improvement |

---

## Master List: MISSING Functions

| # | EVM Function | Severity | Reason Missing | Recommendation |
|---|---|---|---|---|
| 1 | `name()` / `symbol()` | MEDIUM | SPL Token does not store metadata; requires Metaplex Token Metadata | Add `create_metadata` instruction or document manual procedure |
| 2 | `increaseAllowance(spender, addedValue)` | LOW | Convenience function; approve race less relevant on Solana | Low priority; users can re-approve directly |
| 3 | `decreaseAllowance(spender, subtractedValue)` | LOW | Convenience function | Low priority |

---

## Master List: Functions HANDLED BY RUNTIME

| # | EVM Function | Solana Equivalent | How |
|---|---|---|---|
| 1 | `decimals()` | Mint account `decimals` field | Set during init, publicly readable |
| 2 | `totalSupply()` | Mint account `supply` field | Auto-updated on mint/burn by SPL Token |
| 3 | `balanceOf(address)` | Token account `amount` field | Per-account balance storage |
| 4 | `allowance(owner, spender)` | Token account `delegate` + `delegated_amount` | SPL Token delegate model |
| 5 | `owner()` | TokenState PDA `owner` field | On-chain account read |
| 6 | `paused()` | TokenState PDA `is_paused` field | On-chain account read |
| 7 | `minters(address)` | MinterRecord PDA `is_active` field | On-chain account read |
| 8 | `chainId()` | TokenState PDA `evm_chain_id` field | On-chain account read |
| 9 | Proxy upgrade | BPFLoaderUpgradeable | Native Solana program upgrade mechanism |

---

## Top 10 Most Critical Gaps by Severity

| Rank | Gap | Severity | Impact | Status |
|---|---|---|---|---|
| 1 | **Placeholder program ID** | CRITICAL (deploy) | Program will not deploy correctly with placeholder ID | Must fix before deployment |
| 2 | **Bond program integration** | HIGH | Bond program cannot mint LP tokens if it expects to be mint_authority directly; must CPI into lp_token | Requires architecture decision |
| 3 | **Token metadata (name/symbol)** | MEDIUM | Wallets/explorers show mint address instead of token name | Add Metaplex instruction or document procedure |
| 4 | **Burn behavioral difference** | MEDIUM | Bond program must co-sign burns (if it ever needs to burn LP tokens) | Documented improvement; verify bond workflow |
| 5 | **Single-delegate limitation** | LOW | Only one delegate per token account (EVM allows multiple allowances) | Platform limitation; sufficient for use case |
| 6 | **No infinite approval** | LOW | Delegate amount always decrements | Re-approve as needed |
| 7 | **increaseAllowance missing** | LOW | No safe allowance increment helper | Users can approve directly |
| 8 | **decreaseAllowance missing** | LOW | No safe allowance decrement helper | Users can approve to new amount |
| 9 | **MinterRecord rent** | LOW | Deactivated minters still consume rent | Add close instruction in future |
| 10 | **renounceOwnership missing** | INFO | Intentionally omitted | Documented safety restriction |

---

## Recommended Fix Priority Order

### P0 -- Before Deployment
1. **Replace placeholder program ID** (`lib.rs:23`) with real keypair from `solana-keygen`
2. **Resolve bond program integration**: Determine if bond program needs to CPI into lp_token for minting, and if so, modify the bond program's exchange instruction

### P1 -- Before Production Use
3. **Add Metaplex Token Metadata**: Either add a `create_metadata` instruction to the program, or document the manual `mpl-token-metadata create-metadata-v3` procedure to be run after `initialize_mint`

### P2 -- Nice to Have
4. **Add `close_minter_record` instruction**: Allow owner to close deactivated MinterRecord PDAs and reclaim rent
5. Document the single-delegate limitation and re-approval pattern for integrators

### P3 -- Low Priority
6. Consider adding increaseAllowance/decreaseAllowance wrappers if external integrators expect them

---

## Security Improvements in Solana Version

| # | Improvement | EVM Risk Mitigated |
|---|---|---|
| 1 | **Dual-signer burn** | EVM: minters can unilaterally burn any user's tokens. Solana: token holder must co-sign. |
| 2 | **Two-step ownership transfer** | EVM: typo in `transferOwnership` permanently loses governance. Solana: new owner must actively accept. |
| 3 | **No renounceOwnership** | EVM: accidental renunciation permanently disables all governance. Solana: not possible. |
| 4 | **Zero-owner validation on init** | EVM: `initialize(name, symbol, address(0), chainId)` creates ungovernable token. Solana: explicitly rejected. |
| 5 | **PDA-based minter verification** | EVM: relies on mapping read. Solana: PDA derivation + program ownership check + discriminator verification + is_active check (5-layer defense). |
| 6 | **No reentrancy by construction** | EVM: requires explicit ReentrancyGuardUpgradeable. Solana: runtime prevents reentrancy. |

---

## Documented Behavioral Differences (Intentional)

| # | Difference | EVM Value | Solana Value | Reason |
|---|---|---|---|---|
| 1 | Decimals | 18 | 9 | u64 max with 18 decimals = ~18 tokens total. 9 decimals allows ~18.4 billion tokens. |
| 2 | Value range | uint256 (2^256-1) | u64 (2^64-1) | Solana token amounts are u64. With 9 decimals, max ~18.4B tokens. |
| 3 | Burn authorization | Minter/owner only | Minter/owner + token holder | Security improvement |
| 4 | Ownership transfer | One-step | Two-step | Security improvement |
| 5 | Allowance model | Multiple spenders | Single delegate | Solana platform constraint |
| 6 | Event format | Solidity events | Anchor emit!() | Different encoding/indexing |

---

## Files Produced

| File | Description |
|---|---|
| `lptoken-comparison.md` | Function-by-function comparison with 7-dimension analysis |
| `token-model-adaptation.md` | How ERC20 model was adapted to SPL Token |
| `level-differentiation.md` | L1/L2/L3/L4 token level analysis |
| `integration-compatibility.md` | Bond program CPI compatibility analysis |
| `summary.md` | This file -- master summary of all findings |
