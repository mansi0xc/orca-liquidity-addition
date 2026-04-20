# Security Gap Analysis

Comparison between security properties derived from EVM source code and `erc20-security.md`.

---

## SEC-1: Missing — `renounceOwnership` as an attack/risk surface

**Source truth:** `renounceOwnership()` is a public function that permanently sets `_owner = address(0)`. Once called:
- No new minters can be added
- Existing minters cannot be removed
- Contract cannot be paused or unpaused
- Ownership cannot be transferred

**erc20-security.md:** Does not mention `renounceOwnership` at all.

**Risk:** If a compromised owner calls `renounceOwnership`, the contract becomes permanently ungovernable. If a malicious minter exists at that point, they can mint/burn forever with no recourse.

**Status:** NOT addressed in security doc or Solana implementation.

---

## SEC-2: Missing — Freeze authority risk

**Source truth (Solana-specific):** The `initialize_mint` instruction sets `freeze_authority = token_state` PDA. This means the program theoretically has the power to freeze individual token accounts. However, no instruction in the program exercises this power.

**erc20-security.md:** Does not mention freeze_authority at all.

**Risk:** LOW — The authority exists but is never used. An upgrade could add a freeze instruction. Should be documented. The EVM contract has no equivalent freeze mechanism.

**Status:** Missing from security doc.

---

## SEC-3: Missing — Mint authority cannot be transferred/revoked via setAuthority

**Source truth:** erc20-security.md section 6 documents this correctly — it notes that no `transfer_mint_authority` instruction exists and the mint_authority stays as the PDA.

**However,** it does not mention that `setAuthority` could be called by the PDA itself through a CPI if a future instruction were added. The implicit protection is that the program has no such instruction, and the PDA can only sign via the program.

**Status:** Adequately covered in principle. The mitigation is correct.

---

## SEC-4: Missing — `init_if_needed` attack surface on MinterRecord

**Source truth:** `update_minter.rs` uses `init_if_needed` for the MinterRecord PDA. The `init-if-needed` feature is known to have potential front-running risks where an attacker could pre-create an account at the PDA address with different data.

**Mitigation already in place:** The MinterRecord PDA address is deterministic from seeds. Only the system program can allocate at that address. Anchor's `init_if_needed` checks the account discriminator. If the account exists and has the correct discriminator, it's safe.

**erc20-security.md:** Does not mention `init_if_needed` risks.

**Status:** Actually safe in this case. PDA accounts cannot be pre-created by attackers because only the system program can allocate at a PDA. Should still be documented.

---

## SEC-5: Covered — Burn consent (Solana security improvement)

**Source truth:** EVM allows minters to burn from ANY address without consent. Solana implementation requires dual signature (minter + token holder).

**erc20-security.md section 8:** Correctly documents this as a security improvement.

**Status:** ✅ Adequately covered.

---

## SEC-6: Covered — Account substitution prevention

**Source truth:** PDA seeds bind token_state to specific mint. Anchor validates PDA derivation.

**erc20-security.md sections 2, 3, 7:** Correctly documents the PDA binding and seed verification.

**Status:** ✅ Adequately covered.

---

## SEC-7: Covered — CPI safety

**Source truth:** All CPI calls are to the typed `Program<'info, Token>`. Anchor verifies program ID.

**erc20-security.md section 9:** Correctly documents CPI safety.

**Status:** ✅ Adequately covered.

---

## SEC-8: Covered — Reentrancy (N/A on Solana)

**Source truth:** Solana runtime prevents reentrancy by design. CPI to SPL Token does not re-enter lp_token.

**erc20-security.md section 5:** Correctly documents this.

**Status:** ✅ Adequately covered.

---

## SEC-9: Missing — Owner can be set to Pubkey::default() during initialize

**Source truth:** EVM `initialize()` calls `_transferOwnership(owner_)` which has NO zero-address check. The Solana `initialize_mint` also has no check on `params.owner == Pubkey::default()`.

**erc20-security.md:** Does not mention this.

**Risk:** LOW — An operator mistake, not an attack vector. But initializing with Pubkey::default() as owner would create an ungovernable token.

**Status:** NOT addressed.

---

## SEC-10: Missing — `token_mint` not validated against `token_state` in `mint_tokens`/`burn_tokens`

**Source truth:** In `mint_tokens.rs`, the `token_mint` account is `Account<'info, Mint>` with `#[account(mut)]` but no explicit constraint binding it to `token_state`. The binding is **implicit**: the `token_state` PDA is derived from `seeds = [TOKEN_STATE_SEED, token_mint.key().as_ref()]`, so passing a different mint would derive a different PDA that wouldn't match the passed `token_state`. The CPI would also fail because the PDA signer wouldn't be the mint_authority of the wrong mint.

**erc20-security.md:** Partially covers this in section 7 ("Account Validation") but focuses on token_account constraints, not the mint←→token_state binding in mint/burn.

**Status:** Implicitly safe but the security analysis should explicitly call out this implicit binding.

---

## Summary

| ID | Description | Severity | In Security Doc? |
|----|-------------|----------|-----------------|
| SEC-1 | renounceOwnership not analyzed | MEDIUM | No |
| SEC-2 | freeze_authority exists but unused | LOW | No |
| SEC-3 | mint_authority transfer protection | N/A | Yes (adequate) |
| SEC-4 | init_if_needed on MinterRecord | LOW | No |
| SEC-5 | Burn consent improvement | N/A | Yes (adequate) |
| SEC-6 | Account substitution prevention | N/A | Yes (adequate) |
| SEC-7 | CPI safety | N/A | Yes (adequate) |
| SEC-8 | Reentrancy N/A | N/A | Yes (adequate) |
| SEC-9 | Owner can be zero at init | LOW | No |
| SEC-10 | Implicit mint↔token_state binding | INFORMATIONAL | Partially |
