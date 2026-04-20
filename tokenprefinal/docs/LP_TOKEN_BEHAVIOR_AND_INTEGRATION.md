# LP Token — Behavior & Integration Guide

## 1. Overview

The LP Token is a Solana Anchor program (`programs/lp_token/`) that replicates the governance and minting semantics of the EVM LPToken.sol (Energi Core). It wraps the SPL Token Program with access-controlled mint/burn operations while preserving unconstrained transfers and approvals.

**Architecture**:
- Token balances, supply, and allowances are managed entirely by the SPL Token Program
- A `TokenState` PDA per mint holds governance state (owner, pause flag, chain ID)
- `MinterRecord` PDAs track authorized minters per (token_state, minter) pair
- The `TokenState` PDA acts as the SPL `mint_authority`, signing CPI calls to issue tokens

**Key architectural difference from EVM**: The program does not mediate transfers or approvals. It only governs mint, burn, pause, minter registration, and ownership. All standard SPL token operations function independently of this program.

---

## 2. Core Behavioral Guarantees

| Behavior | Guarantee |
|----------|-----------|
| **Minting** | Only `token_state.owner` or a registered minter (`is_active == true`) may mint. Blocked when paused. |
| **Burning** | Only owner or registered minter as `authority`, AND the token account holder must co-sign as `token_account_authority`. Blocked when paused. |
| **Pause scope** | Pause blocks `mint_tokens` and `burn_tokens` ONLY. Transfers, approvals, and all direct SPL operations are unaffected. |
| **Ownership** | Two-step: propose via `transfer_ownership`, finalize via `accept_ownership`. Owner retains full control until acceptance. |
| **Supply** | No maximum supply cap. Unbounded minting is intentional (matches EVM LPToken). |
| **Metadata** | Name, symbol, and URI are stored via Metaplex Token Metadata CPI. Owner-only. |

---

## 3. Critical Nuances

### 3.1 Dual-Signer Burn — Scope and Limitations

The burn instruction requires two signers:
1. `authority` — must be owner or registered minter
2. `token_account_authority` — must be the owner of the token account being burned from

**This protection applies ONLY to user-owned (EOA) token accounts.**

When a program (e.g., the bond program) owns a vault token account via a PDA:
- The program's PDA can act as BOTH `authority` (if registered as minter) and `token_account_authority`
- Only ONE PDA signature is needed (the runtime sees both accounts as the same signer)
- The "dual-signer" protection collapses — the program can burn from its vault unilaterally

**This is intentional.** In the bond redemption flow, the bond program CPI-burns LP tokens from its vault when a user redeems. The user's signature is enforced by the bond program's own logic, not by the LP Token program.

### 3.2 SPL Token Program Bypass

Users and programs can call the SPL Token Program directly to:
- Transfer tokens (`spl_token::transfer`)
- Approve delegates (`spl_token::approve`)
- Revoke delegates (`spl_token::revoke`)
- Close accounts (`spl_token::close_account`)

The LP Token program's `transfer_tokens` and `approve_delegate` instructions are **convenience wrappers**, not enforcement gates. The program has no freeze authority and cannot block direct SPL operations.

**Implications**:
- The program emits no events for direct SPL transfers
- Indexers MUST monitor both the LP Token program AND the SPL Token program for complete visibility
- There is no on-chain way to restrict LP token transfers (by design — matches EVM LPToken which has no `_transfer` override)

### 3.3 Delegate (Allowance) Behavior

SPL Token delegates differ from ERC20 allowances:

| ERC20 | SPL Token |
|-------|-----------|
| Multiple spenders per owner | ONE delegate per token account |
| Allowance persists at 0 after full spend | Delegate field cleared after full spend |
| `approve(spender, type(uint256).max)` for infinite approval | No infinite approval — capped at u64 |
| `transferFrom` reads allowance mapping | SPL checks `delegated_amount` on account |

After a delegate spends their full allowance, the token account's `delegate` field becomes `None`. Any subsequent delegated transfer will fail until a new `approve` is issued.

### 3.4 Token Account Lifecycle

- Users may hold LP tokens in multiple token accounts (not just the canonical ATA)
- Token accounts with zero balance can be closed by their owner, reclaiming rent
- Closing an account permanently removes it — there is no on-chain history that the user ever held tokens
- Unlike EVM, there is no persistent `balanceOf` mapping that retains zero-balance entries

---

## 4. Metadata Behavior

- Token metadata (name, symbol, URI) is stored in a Metaplex Token Metadata account, NOT in the SPL mint
- The `set_metadata` instruction creates or updates metadata via CPI to the Metaplex program
- Metadata must be set **after** mint initialization (separate instruction)
- The `token_state` PDA is set as both `mint_authority` and `update_authority` on the metadata account
- Only the program owner can call `set_metadata`; the PDA signs the Metaplex CPI

**Length limits** (enforced by Metaplex, not by this program):
- `name`: max 32 characters
- `symbol`: max 10 characters  
- `uri`: max 200 characters

**Environment requirement**: The Metaplex Token Metadata program (`metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s`) must be deployed on the target cluster. It is available on mainnet and devnet but NOT on default localnet validators.

---

## 5. Access Control Model

### Roles

| Role | Capabilities |
|------|-------------|
| **Owner** (`token_state.owner`) | Mint, burn, update minters, pause/unpause, transfer ownership, set metadata |
| **Minter** (`MinterRecord.is_active == true`) | Mint, burn (with token holder co-sign) |
| **Token holder** | Transfer, approve delegates, co-sign burns |

### Minter Verification (`verify_minter`)

When `authority` is not the owner, the program validates:
1. The passed `minter_record` address matches the PDA derived from `[b"minter", token_state, authority]`
2. The account is owned by this program (prevents cross-program spoofing)
3. The account is initialized (has data)
4. Deserialization succeeds (Anchor discriminator check)
5. `is_active == true`

A deregistered minter (`is_active == false`) cannot mint or burn, even though their MinterRecord PDA still exists on-chain.

### Ownership Transfer Does NOT Reset Minters

When ownership transfers to a new address:
- All existing MinterRecords remain active
- The new owner inherits the full set of registered minters
- The new owner must manually deregister unwanted minters via `update_minter`

---

## 6. Integration Guidelines

### 6.1 Bond Program Integration

**Registering as a minter:**
```
1. Owner calls update_minter(bond_program_pda, is_active: true)
2. Bond program's PDA is now authorized to mint/burn
```

**Minting LP tokens to a user:**
```
Bond program CPIs into mint_tokens:
  authority: bond_program_pda (signer via PDA seeds)
  minter_record: derived from [b"minter", token_state, bond_program_pda]
  recipient_token_account: user's ATA
```

**Burning LP tokens from a vault:**
```
Bond program CPIs into burn_tokens:
  authority: bond_program_pda (minter)
  token_account_authority: bond_program_pda (vault owner — same PDA)
  token_account: vault token account owned by bond_program_pda
```

The dual-signer requirement is satisfied by a single PDA signature because both accounts reference the same key. The user's consent is enforced by the bond program's own instruction logic (e.g., requiring the user to sign the redemption instruction).

**Critical**: The bond program MUST validate that the user actually wants to redeem. The LP Token program does not protect vault accounts from their owning program.

### 6.2 Frontend Considerations

| Concern | Handling |
|---------|----------|
| Decimals | Use 9 (not 18). 1 LP token = 1,000,000,000 base units. |
| Displaying metadata | Fetch the Metaplex metadata PDA at `["metadata", metaplex_program_id, mint]` |
| Approval flow | After a full delegated transfer, re-approve is required (delegate is cleared) |
| Balance queries | Read the user's ATA directly via SPL — no program RPC needed |
| Supply queries | Read the mint account's `supply` field |
| Pause status | Fetch `TokenState` PDA and check `is_paused` |

### 6.3 Indexer Considerations

**Event sources to monitor:**

| Source | Events |
|--------|--------|
| LP Token program | `TokensMinted`, `TokensBurned`, `MinterUpdated`, `PauseStateChanged`, `OwnershipTransferProposed`, `OwnershipTransferred`, `MetadataSet` |
| SPL Token program | All `Transfer`, `Approve`, `Revoke`, `CloseAccount` instructions affecting this mint |

**Multi-mint disambiguation**: The `MinterUpdated` event includes a `token_state` field. Use this to associate minter changes with specific mints when the same address is a minter on multiple LP token instances.

**Same-slot ordering**: Multiple minters submitting transactions in the same slot will have events ordered by the validator's transaction scheduling. Use `slot + transaction_index + instruction_index` as a canonical ordering key.

---

## 7. Known Differences from EVM

| # | EVM LPToken | Solana LP Token | Reason |
|---|-------------|-----------------|--------|
| 1 | `burn(address, uint256)` — unilateral | Dual-signer: minter + token holder | Security improvement for user accounts |
| 2 | `transferOwnership` — one step | Two-step propose/accept | Prevents accidental ownership loss |
| 3 | `renounceOwnership()` available | Blocked (not implemented) | Prevents permanent governance loss |
| 4 | `name()` / `symbol()` set at init | Set post-init via `set_metadata` | SPL mints don't store metadata natively |
| 5 | Multiple approvals per owner | One delegate per token account | SPL Token model limitation |
| 6 | 18 decimals | 9 decimals | u64 overflow constraint (max ~18.4B tokens) |
| 7 | Freeze not applicable | Freeze authority explicitly set to None | Least-privilege (EVM has no freeze concept) |
| 8 | `_balances` mapping persists | Token accounts can be closed | SPL rent-reclaim model |
| 9 | `nonReentrant` modifier | Not needed | Solana runtime prevents reentrancy |
| 10 | ProxyAdmin controls upgrades | Solana upgrade authority | Different upgrade mechanism |

---

## 8. Security Considerations

### Reentrancy
Not possible. The Solana runtime does not allow a program to be re-invoked within its own call stack without explicit CPI recursion (which this program does not perform).

### PDA Validation
All minter checks derive the expected PDA and compare against the passed account. Combined with program-owner verification and discriminator checks, this prevents:
- Cross-program account spoofing
- Stale/fake minter records
- Account substitution attacks

### Metaplex CPI Safety
- The metadata PDA is validated via `seeds::program = TOKEN_METADATA_PROGRAM_ID` (Anchor constraint)
- The Metaplex program ID is verified via `#[account(address = ...)]`
- The `token_state` PDA signs as both `mint_authority` and `update_authority`
- No external party can create or modify metadata without the PDA signature

### Upgrade Authority
Until the upgrade authority is transferred to a multisig or finalized, a single key can deploy arbitrary new code. This is the highest operational risk and must be mitigated immediately post-deployment.

### Token Program Validation
Both `mint_tokens` and `burn_tokens` validate that `token_mint.mint_authority == token_state.key()`. This prevents using the program against a mint it doesn't control.

---

## 9. Operational / Deployment Notes

| Step | Command / Action |
|------|-----------------|
| Generate program keypair | `solana-keygen new -o target/deploy/lp_token-keypair.json` |
| Set program ID | Copy pubkey into `declare_id!()` in `lib.rs` |
| Deploy | `anchor deploy --provider.cluster mainnet` |
| Transfer upgrade authority | `solana program set-upgrade-authority <PROGRAM_ID> --new-upgrade-authority <MULTISIG>` |
| Initialize mint | Call `initialize_mint` with owner, chain_id, decimals=9 |
| Set metadata | Call `set_metadata` with name, symbol, URI |
| Register minters | Call `update_minter` for each authorized minter (e.g., bond program PDA) |
| (Optional) Finalize | `solana program set-upgrade-authority <PROGRAM_ID> --final` — IRREVERSIBLE |

---

## 10. Non-Issues (Clarifications)

The following behaviors may appear problematic but are expected and correct:

| Behavior | Why It's Fine |
|----------|---------------|
| Program-owned vaults can burn without user signature | By design. The owning program enforces user consent via its own instruction logic. |
| Users can transfer LP tokens directly via SPL Token | By design. LPToken EVM contract also does not restrict transfers. |
| Ownership transfer does not revoke minters | Matches EVM. New owner can deregister unwanted minters manually. |
| Token accounts can be closed (losing balance history) | Normal SPL behavior. On-chain history is preserved in transaction logs. |
| `init_if_needed` on MinterRecord | Safe. PDA accounts cannot be front-run. Transaction rollback is atomic if the instruction fails. |
| `update_minter(address, false)` fails for never-registered address | Matches EVM (`require(minters[_account] != _isMinter)` also reverts). |
| Metadata creation fails on localnet | Expected. Metaplex program is only deployed on devnet/mainnet. |
| Zero-amount mint/burn succeeds | Matches ERC20 behavior. No-op with correct event emission. |
