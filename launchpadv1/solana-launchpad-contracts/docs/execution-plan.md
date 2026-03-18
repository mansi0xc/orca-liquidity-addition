# Execution Plan — Solana Launchpad Implementation

> Phase 4: Detailed implementation plan with tasks, dependencies, and risks

---

## Architecture Decision

Rather than implementing 6 separate programs (mirroring the 6 EVM contracts), we will implement **2 Anchor programs**:

1. **`gmi_launchpad`** — The main NFT Launchpad program handling all 3 collection types (Standard, Refundable100, Refundable80) with an optional OperatorFilter feature
2. **`operator_registry`** — The OperatorRegistry program for marketplace whitelist and revenue sharing

This consolidation is justified because:
- The 6 EVM contracts share 80%+ code — duplication is the EVM pattern, not the protocol design
- A `CollectionType` enum elegantly selects behavior at runtime
- `has_operator_filter` boolean enables/disables Creator Economy features
- Single program = simpler upgrades, audits, and deployments
- ALL functionality is preserved — zero feature loss

---

## Phase A: Project Structure Setup

### Tasks
1. Initialize Anchor workspace with 2 programs
2. Set up directory structure (no `mod.rs` files)
3. Configure `Anchor.toml` and `Cargo.toml`
4. Set up module structure per program

### File Structure
```
programs/
  gmi-launchpad/
    src/
      lib.rs                    # Entrypoint + declare_id! + program module
      state.rs                  # All account structs + enums
      errors.rs                 # Custom error codes
      events.rs                 # Event structs
      utils.rs                  # Helper functions
      instructions/
        initialize_collection.rs
        mint_public.rs
        mint_presale.rs
        mint_owner.rs
        refund_nft.rs
        configure_publicsale.rs
        configure_presale.rs
        toggle_presale.rs
        toggle_publicsale.rs
        toggle_pause.rs
        set_base_uri.rs
        add_whitelist.rs
        remove_whitelist.rs
  operator-registry/
    src/
      lib.rs
      state.rs
      errors.rs
      events.rs
      instructions/
        initialize_registry.rs
        add_operator_whitelist.rs
        remove_operator_whitelist.rs
        add_universal_operator.rs
        remove_universal_operator.rs
        change_fund_receiver.rs
        change_share_percentage.rs
        toggle_registry_pause.rs
```

### Dependencies
- `anchor-lang` = "0.30.1"
- `anchor-spl` = "0.30.1"
- `mpl-token-metadata` (latest)

### Risks
- Metaplex version compatibility
- Account space calculation for dynamic fields (Vec, String)

---

## Phase B: State Accounts

### Tasks
1. Define `Collection` account struct with all fields
2. Define `MintCounter` account struct
3. Define `WhitelistEntry` account struct
4. Define `TokenRecord` account struct
5. Define `CollectionType` enum
6. Define vault PDA (system-owned, no struct needed)
7. Define `OperatorRegistryState` account struct
8. Define `OperatorWhitelist` account struct
9. Define `UniversalOperator` account struct

### Dependencies
- Phase A complete

### Risks
- Account size limits (10 KB max per account)
- `Vec<u64>` for refunded_token_ids may exceed limits — may need separate account or capped vector

---

## Phase C: Core Instructions

### Tasks (in dependency order)
1. `initialize_collection` — Create collection with all parameters
2. `mint_public` — Public sale minting with payment + NFT creation
3. `mint_presale` — Presale minting with whitelist validation
4. `mint_owner` — Authority minting (no payment)
5. `refund_nft` — Token burn + SOL refund from vault
6. `configure_publicsale` / `configure_presale` — Update sale parameters
7. `toggle_presale` / `toggle_publicsale` / `toggle_pause` — Status toggles
8. `set_base_uri` — Metadata URI update
9. `add_whitelist` / `remove_whitelist` — Whitelist management

### Dependencies
- Phase B complete
- State structs defined
- Error codes defined

### Risks
- Correct CPI construction for token mint/burn
- Vault PDA signing for refund SOL transfers
- 80/20 split precision with integer math

---

## Phase D: Access Control

### Tasks
1. Implement authority checks on all admin instructions
2. Implement pause checks on all mint instructions
3. Implement whitelist verification for presale
4. Implement token ownership verification for refund
5. Implement operator registry checks for C-variant features

### Dependencies
- Phase C core instructions exist

### Risks
- Missing authority check = privilege escalation
- Missing pause check = bypassing admin controls

---

## Phase E: Validation Layers

### Tasks
1. Add account constraint validation on all contexts
2. Add PDA seed verification on all PDA accounts
3. Add `has_one` constraints linking accounts
4. Add explicit program ID checks for CPIs
5. Add overflow checks on all arithmetic
6. Add vault balance checks before refund transfers

### Dependencies
- Phases C + D complete

### Risks
- Over-constraining may cause legitimate transactions to fail
- Under-constraining may allow exploits

---

## Phase F: Error Codes & Events

### Tasks
1. Define comprehensive error enum mirroring all EVM require conditions
2. Define event structs for all EVM events
3. Add `emit!()` calls to all instructions
4. Ensure event parity with EVM (fix toggle event bug)

### Dependencies
- Can be done in parallel with Phase C

---

## Phase G: Operator Registry Program

### Tasks
1. `initialize_registry` — Set up registry with fund receiver + share BPS
2. `add_operator_whitelist` / `remove_operator_whitelist` — Per-collection marketplace whitelist
3. `add_universal_operator` / `remove_universal_operator` — Global marketplace whitelist
4. `change_fund_receiver` / `change_share_percentage` — Admin config
5. `toggle_registry_pause` — Pause/unpause
6. Integration: Launchpad CPI to registry for transfer validation

### Dependencies
- Phase A structure

### Risks
- CPI between programs must be carefully constructed
- Registry state must be readable from launchpad program

---

## Phase H: Testing

### Tasks
1. Unit tests for each instruction (happy path)
2. Unit tests for access control (negative cases)
3. Unit tests for all 3 collection types
4. Integration tests for mint → refund lifecycle
5. Integration tests for presale whitelist flow
6. Security tests (account substitution, unauthorized access)
7. Edge case tests (max supply, zero price, overflow)

### Dependencies
- All implementation phases complete

### Risks
- Test environment differences from mainnet
- Metaplex program deployment in test

---

## Timeline Estimate

| Phase | Effort | Dependencies |
|---|---|---|
| A: Structure Setup | Small | None |
| B: State Accounts | Small | A |
| C: Core Instructions | Large | B |
| D: Access Control | Medium | C |
| E: Validation | Medium | C, D |
| F: Errors & Events | Small | Parallel with C |
| G: Operator Registry | Medium | A |
| H: Testing | Large | All |
