# Security Migration — EVM → Solana

> Phase 3: Comprehensive security model translation with Solana-specific protections

---

## 1. EVM Security Checks → Solana Equivalents

| # | EVM Check | EVM Implementation | Solana Equivalent | Solana Implementation |
|---|---|---|---|---|
| S1 | Reentrancy Guard | `nonReentrant` modifier | Not needed | Solana runtime prevents reentrancy by program design |
| S2 | Ownership Check | `onlyOwner` modifier | Authority constraint | `has_one = authority` on collection account |
| S3 | Pause Check | `whenNotPaused` modifier | Manual require | `require!(!collection.paused, LaunchpadError::Paused)` |
| S4 | Contract Caller Block | `noContracts` / `Address.isContract` | CPI Guard | Check that instruction is not invoked via CPI: `require!(ctx.accounts.instruction_sysvar... )` or simply trust Solana's signer model |
| S5 | Value Validation | `require(msg.value == qty * price)` | Lamport transfer check | Verify SOL transfer amount matches expected via `system_program::transfer` |
| S6 | Supply Cap | `require(mintedAmount + qty <= maxMintSupply)` | Same logic | `require!(collection.minted_amount + quantity <= collection.max_mint_supply, ...)` |
| S7 | Per-User Limit | `require(userMints + qty <= maxUserMintAmount)` | Same logic + PDA | Check `mint_counter.number_minted + quantity <= collection.max_user_mint_amount` |
| S8 | Per-Tx Limit | `require(qty <= maxTxMintAmount)` | Same logic | `require!(quantity <= collection.max_tx_mint_amount, ...)` |
| S9 | Whitelist Check | `require(whitelists[sender] > 0)` | PDA existence + value | Verify `whitelist_entry` PDA exists and `mint_limit > 0` |
| S10 | Token Ownership (Refund) | `require(sender == ownerOf(tokenId))` | Token account check | Verify signer owns the token account holding the NFT |
| S11 | Owner-Mint Non-Refundable | `require(!isOwnerMint[tokenId])` | TokenRecord check | `require!(!token_record.is_owner_mint, ...)` |
| S12 | Free NFT Non-Refundable | `require(refundPrice[tokenId] > 0)` | TokenRecord check | `require!(token_record.refund_price > 0, ...)` |
| S13 | Transfer Success | `require(os, "Transfer failed")` | CPI result check | System program transfer will fail transaction on insufficient funds |
| S14 | Array Length Match | `require(users.length == limits.length)` | Instruction validation | Validate via remaining accounts or instruction data |
| S15 | Zero Address Check | `require(user != address(0))` | Not needed | Solana has no zero-address concept; use `Pubkey::default()` check if needed |

---

## 2. Solana-Specific Risks & Mitigations

### Risk 1: Account Substitution Attacks
**Description**: Attacker passes forged accounts with same data layout but different ownership.
**Mitigation**:
- All PDA accounts MUST have `seeds` and `bump` constraints in Anchor
- Use `has_one` constraints to link accounts (e.g., `collection` on `MintCounter`)
- Verify `token_record.collection == collection.key()` explicitly
- Use `Account<'info, T>` type checking (Anchor discriminator)

### Risk 2: CPI Abuse
**Description**: Malicious program invokes launchpad instructions via CPI.
**Mitigation**:
- Check `ctx.accounts.instructions_sysvar` to detect CPI context when needed
- All token operations MUST verify target program is `spl_token::id()` or `token_2022::id()`
- System program transfers MUST verify `system_program::id()`

### Risk 3: Unchecked Remaining Accounts
**Description**: Extra accounts passed could be misused.
**Mitigation**:
- For batch operations (whitelist add), iterate remaining accounts with explicit PDA derivation
- NEVER trust data from remaining accounts without PDA verification
- Document expected remaining accounts count per instruction

### Risk 4: PDA Spoofing
**Description**: Attacker derives PDA with different seeds that collides.
**Mitigation**:
- All PDA seeds MUST include unique identifiers (collection pubkey, user pubkey)
- Store bump in account and verify on subsequent access
- Use `seeds::program = program_id` constraint

### Risk 5: Signer Spoofing
**Description**: Transaction includes non-signer account in signer slot.
**Mitigation**:
- All authority operations MUST use `Signer<'info>` type
- Minter account MUST be `Signer<'info>`
- PDA signing uses `seeds` + `bump` for program-derived authorities

### Risk 6: Rent / Lamport Edge Cases
**Description**: Account drained below rent-exempt minimum, leading to account deletion.
**Mitigation**:
- All accounts MUST be rent-exempt (Anchor handles this automatically for `init`)
- Vault PDA MUST maintain rent-exempt balance after refund transfers
- Use `min_lamports = rent_exempt_minimum` assertion before SOL transfers from vault

### Risk 7: Token Account Manipulation
**Description**: Attacker uses wrong token account or mint account.
**Mitigation**:
- Token account MUST have `constraint = token_account.mint == mint.key()`
- Token account MUST have `constraint = token_account.owner == owner.key()`
- Mint account MUST match the one recorded in `TokenRecord`

### Risk 8: Double Refund
**Description**: Same NFT refunded twice.
**Mitigation**:
- On refund, burn the token (close mint authority, close token account)
- Close `TokenRecord` PDA (returns rent)
- Token mint account becomes unusable after burn

### Risk 9: Integer Overflow/Underflow
**Description**: Arithmetic operations overflow.
**Mitigation**:
- Rust uses checked arithmetic by default in debug mode
- Use `checked_add`, `checked_sub`, `checked_mul` for critical calculations
- 80/20 split: `price.checked_mul(80).unwrap().checked_div(100).unwrap()`

### Risk 10: Unauthorized Vault Drain
**Description**: Attacker drains refund vault.
**Mitigation**:
- Vault PDA can only be signed by the program
- Only `refund_nft` instruction can withdraw from vault
- Verify refund amount matches `token_record.refund_price` exactly

---

## 3. Required Protections Per Component

### Collection Account
- [x] PDA seeds verified: `["collection", collection_id]`
- [x] `has_one = authority` on all admin operations
- [x] Discriminator checked (automatic with Anchor `Account<T>`)
- [x] Owner checked (automatic with Anchor — program owns it)

### MintCounter Account
- [x] PDA seeds verified: `["mint_counter", collection.key(), user.key()]`
- [x] `constraint = mint_counter.collection == collection.key()`
- [x] Init-if-needed for first mint
- [x] Only modified during mint operations

### WhitelistEntry Account
- [x] PDA seeds verified: `["whitelist", collection.key(), user.key()]`
- [x] Only created/modified by authority
- [x] Closed on removal (rent returned to authority)

### TokenRecord Account
- [x] PDA seeds verified: `["token_record", collection.key(), mint.key()]`
- [x] Created during mint, closed during refund
- [x] `refund_price` set once at creation, never modified

### Vault PDA
- [x] Seeds: `["vault", collection.key()]`
- [x] Only debited during `refund_nft` instruction
- [x] Only credited during mint instructions
- [x] Rent-exempt minimum enforced

---

## 4. Invariants That MUST NEVER Break

| # | Invariant | Enforcement |
|---|---|---|
| INV1 | `collection.minted_amount <= collection.max_mint_supply` | Checked in every mint instruction |
| INV2 | `token_record.is_owner_mint == true → refund is denied` | Checked in `refund_nft` |
| INV3 | `token_record.refund_price is immutable after creation` | Never modified, only read |
| INV4 | `vault.lamports >= Σ(refund_price for all live refundable NFTs)` | Ensured by not allowing withdrawal except via refund |
| INV5 | `collection.total_mints is monotonically increasing` | Only incremented in mint, never decremented |
| INV6 | `mint_counter values >= 0 (no underflow)` | Use checked arithmetic |
| INV7 | `One MintCounter per (collection, user) pair` | Enforced by PDA seeds |
| INV8 | `One WhitelistEntry per (collection, user) pair` | Enforced by PDA seeds |
| INV9 | `One TokenRecord per (collection, mint) pair` | Enforced by PDA seeds |
| INV10 | `Authority cannot be zero/default pubkey` | Checked on initialization and authority change |

---

## 5. Per-Instruction Security Checklist

### `initialize_collection`
- [ ] Authority is signer
- [ ] Collection PDA derived correctly
- [ ] Initial values validated (max_supply > 0, price >= 0)
- [ ] Bump stored
- [ ] Collection type determines which features are enabled

### `mint_public`
- [ ] Collection not paused
- [ ] Public sale active
- [ ] Quantity > 0 and <= max_tx_mint_amount
- [ ] mint_counter.number_minted + quantity <= max_user_mint_amount
- [ ] minted_amount + quantity <= max_mint_supply
- [ ] SOL payment exactly matches expected amount
- [ ] Token record created with correct refund_price
- [ ] mintedAmount incremented
- [ ] SOL transferred to correct destination (owner or vault)

### `mint_presale`
- [ ] Collection not paused
- [ ] Presale active
- [ ] Whitelist entry exists and mint_limit > 0
- [ ] presale_number_minted + quantity <= whitelist.mint_limit
- [ ] All checks from `mint_public` apply
- [ ] Free presale: quantity <= reserved_nfts - reserved_mints
- [ ] presaleNumberMinted incremented

### `mint_owner`
- [ ] Caller is authority
- [ ] minted_amount + quantity <= max_mint_supply
- [ ] Token record marked is_owner_mint = true
- [ ] Reserved mints tracked (for R variants)
- [ ] NO payment required
- [ ] NO noContracts check (authority can be program)

### `refund_nft`
- [ ] Collection type is Refundable100 or Refundable80
- [ ] Caller owns the NFT (token account owner == signer)
- [ ] Token record exists and is_owner_mint == false
- [ ] refund_price > 0
- [ ] Token burned (mint authority revoked, token account closed)
- [ ] SOL transferred from vault to signer
- [ ] Token record closed (rent returned)
- [ ] mintedAmount decremented
- [ ] refundCounter incremented

### `add_whitelist` / `remove_whitelist`
- [ ] Caller is authority
- [ ] User pubkey is not default
- [ ] PDA derived correctly for (collection, user)

### `configure_publicsale` / `configure_presale`
- [ ] Caller is authority
- [ ] Collection not paused (if required)
- [ ] Values are reasonable (no overflow possible)

### `toggle_presale` / `toggle_publicsale` / `toggle_pause`
- [ ] Caller is authority
- [ ] State toggled correctly
- [ ] Event emitted with NEW state (fix EVM bug)

### OperatorRegistry Instructions
- [ ] All admin instructions: authority is signer
- [ ] Registry not paused for modification
- [ ] Fund receiver is not default pubkey
- [ ] Share percentage BPS <= 10000 (100%)
