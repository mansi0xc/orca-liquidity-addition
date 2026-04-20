# Level Differentiation: L1/L2/L3/L4 Token Analysis

**Date**: 2026-03-31

---

## EVM Token Levels

The EVM codebase contains three distinct token implementations, each representing a different level of functionality:

### LPToken (Migration Target)
- **Supply**: Uncapped (no maxSupply)
- **Pause scope**: Mint/burn only -- transfers/approvals always work
- **Transfer restrictions**: None
- **Special features**: None
- **Role**: Liquidity provider token for LP bond system

### GMIToken
- **Supply**: Capped at `maxSupply` (set at initialization, immutable)
- **Pause scope**: ALL operations -- mint, burn, transfer, approve all blocked
- **Transfer restrictions**: None beyond pause
- **Special features**: `_transfer` and `_approve` overridden with `whenNotPaused` + `nonReentrant`
- **Bug**: `transferFrom()` broken for finite allowances due to double `nonReentrant` on `_transfer` + `_approve`

### GMICVToken
- **Supply**: Capped at `maxSupply` (updateable by owner)
- **Pause scope**: ALL operations
- **Transfer restrictions**: Trading restrictions -- when `tradeAllowed == false`, contracts must be in `allowedExchanges` whitelist
- **Special features**: `allowTrade()`, `updateAllowedExchange()`, `updateMaxSupply()`, `_isContract()` helper
- **Bug**: Same `transferFrom()` bug as GMIToken

---

## Solana Implementation

The Solana program (`programs/lp_token/`) implements **only the LPToken** level. This is the correct scope since the migration target is specifically LPToken.

### What is implemented (LPToken level)
- Uncapped supply: No maxSupply check in `mint_tokens`
- Pause blocks mint/burn only: constraint on `token_state.is_paused` only in `MintTokens` and `BurnTokens`
- No transfer restrictions: `transfer_tokens` has no pause check, no whitelist
- Minter registry: MinterRecord PDAs for authorized minters

### What is NOT implemented (GMIToken level features)
- `maxSupply` field and enforcement
- Pause-gated transfers (no `_transfer` override equivalent)
- Pause-gated approvals (no `_approve` override equivalent)

### What is NOT implemented (GMICVToken level features)
- `maxSupply` with `updateMaxSupply`
- `allowedExchanges` whitelist
- `tradeAllowed` toggle
- `allowTrade()` instruction
- `updateAllowedExchange()` instruction
- Contract detection (`_isContract` equivalent)

---

## Assessment

The Solana implementation correctly targets only the LPToken level. The GMIToken and GMICVToken levels are separate token contracts on EVM and would require separate Solana programs if they need to be migrated.

**No levels are missing or incorrectly implemented for the LPToken migration scope.**

If GMIToken or GMICVToken migration is planned in the future:
- GMIToken would require adding: `maxSupply` to TokenState, supply check in mint, and transfer/approve restrictions when paused (potentially via Token-2022 freeze authority or custom transfer hook)
- GMICVToken would additionally require: exchange whitelist PDA accounts, trade toggle, and contract detection logic (which has no direct Solana equivalent since all Solana accounts are either programs or data accounts)
