# Integration Compatibility: lp_token <-> LP Bonds Programs

**Date**: 2026-03-31
**LP Token Program**: `/solana-token/programs/lp_token/`
**Bond Programs**: `/solana-lp-bonds-contracts/programs/lp-bonds/` and `/solana-lp-bonds-contracts/programs/lp-bonds-evolution/`

---

## Bond Program Architecture

The LP bonds contracts at `/solana-lp-bonds-contracts/` contain two programs:
1. **lp-bonds**: Main bond program (minting bonds, exchanging, fee collection, recovery)
2. **lp-bonds-evolution**: Bond evolution functionality

### How Bond Programs Interact with LP Tokens

The bond programs do **NOT** CPI into the `lp_token` program. Instead, they interact with LP tokens in two ways:

#### 1. Direct SPL Token CPI (mint_to)
When exchanging bonds for LP tokens, the bond program calls `token::mint_to` directly via CPI:
```rust
// lp-bonds/src/lib.rs:958-968
token::mint_to(
    CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        MintTo {
            mint: ctx.accounts.destination_token_mint.to_account_info(),
            to: ctx.accounts.user_destination_token_account.to_account_info(),
            authority: ctx.accounts.exchange_mint_authority.to_account_info(),
        },
        signer_seeds,
    ),
    amount_out,
)?;
```

The `exchange_mint_authority` PDA (seeds: `["exchange_mint_authority"]`) is used as the mint authority signer for these operations.

#### 2. Direct SPL Token CPI (transfer)
The bond program transfers tokens directly via SPL Token transfer CPI using the `bond_authority` PDA.

---

## Compatibility Analysis

### Critical Finding: Mint Authority Conflict

The lp_token program sets the **token_state PDA** as the sole `mint_authority` on the SPL mint during initialization (`initialize_mint.rs:36`):
```rust
mint::authority = token_state,
```

The bond program expects its own **exchange_mint_authority PDA** to be the mint authority for destination tokens (LP tokens).

**These two approaches are mutually exclusive.** An SPL mint can have exactly one `mint_authority`. If the token_state PDA is the mint_authority, the bond program's `exchange_mint_authority` cannot mint tokens directly.

### Resolution Options

#### Option A: Register Bond Program's PDA as a Minter (Recommended)
The bond program's `exchange_mint_authority` PDA should be registered as a minter in the lp_token program via `update_minter`. Then the bond program would need to CPI into `lp_token::mint_tokens` instead of directly into SPL Token `mint_to`.

**Requires**: Bond program must be modified to CPI into lp_token instead of SPL Token directly for minting.

**Account structure for CPI**:
- `authority`: exchange_mint_authority PDA (signer via bond program's PDA seeds)
- `token_state`: the token_state PDA for the LP token mint
- `minter_record`: the MinterRecord PDA for exchange_mint_authority
- `token_mint`: the LP token mint
- `recipient_token_account`: user's LP token ATA
- `token_program`: SPL Token program

#### Option B: Set Bond Program's PDA as Mint Authority Directly
Instead of using token_state as mint_authority, set the bond program's exchange_mint_authority as the mint_authority. This breaks the lp_token program's ability to mint.

**Not recommended**: This defeats the purpose of the lp_token governance layer.

#### Option C: Transfer Mint Authority After Initialization
Use SPL Token's `set_authority` to transfer mint_authority from token_state PDA to the bond program's PDA. This removes lp_token program governance entirely.

**Not recommended** for same reason as Option B.

#### Option D: lp_token Program Mints on Behalf of Bond Program
Keep token_state as mint_authority. The bond program CPIs into lp_token's `mint_tokens` instruction, which then CPIs into SPL Token's `mint_to`. The bond program's exchange_mint_authority is registered as a minter.

**This is the cleanest approach** and is how the system was likely intended to work.

---

## Interface Compatibility Check

### What Bond Programs Need from LP Token

| Operation | Bond Program Expectation | lp_token Provides | Compatible? |
|---|---|---|---|
| Mint LP tokens to user | CPI to mint with authority | `mint_tokens` instruction | YES (if bond program CPIs into lp_token) |
| Transfer tokens | Direct SPL Token transfer | SPL Token (no lp_token involvement) | YES |
| Read token balance | SPL Token account read | SPL Token account | YES |
| Read supply | SPL Mint account read | SPL Mint account | YES |
| Pause check | Not checked by bond programs | N/A | YES |

### What Bond Programs Do NOT Need
- Burn LP tokens (bond programs burn bond NFTs, not LP tokens)
- Approve/delegate LP tokens
- Manage minters
- Manage pause state

---

## Action Items for Integration

### Required Changes (if using Option D)

1. **In bond program**: Modify the exchange instruction to CPI into `lp_token::mint_tokens` instead of direct `token::mint_to`
   - Add lp_token program ID to the instruction accounts
   - Add token_state PDA account
   - Add minter_record PDA account for exchange_mint_authority

2. **Deployment procedure**: After deploying both programs:
   - Initialize LP token mint via `initialize_mint`
   - Register bond program's `exchange_mint_authority` PDA as a minter via `update_minter`
   - The exchange_mint_authority PDA key must be computed using the bond program's ID and seeds

### No Changes Needed If
- The bond program already accounts for the lp_token program as a governance layer
- The bond program mints its own separate token (not the same mint managed by lp_token)

---

## Current Status

Based on code inspection, the bond programs at `/solana-lp-bonds-contracts/` do not reference `lp_token`, `token_state`, or any lp_token-specific constants. This suggests either:
1. The bond programs are designed to work with a simple SPL mint where they control the mint authority directly (pre-lp_token migration)
2. The integration has not yet been implemented

**Recommendation**: Clarify with the team whether the bond programs' `destination_token_mint` (used in exchange operations) is intended to be the same LP token governed by the lp_token program. If yes, the bond program needs modification to CPI into lp_token for minting.
