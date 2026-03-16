# Security Model — EVM → Solana Translation

## Overview

This document maps every EVM security mechanism in LPToken to its Solana Anchor equivalent,
and identifies new threat vectors introduced by the Solana account model.

---

## 1. Caller Identity: `msg.sender` → `Signer<'info>`

### EVM
```solidity
require(msg.sender == owner(), "Ownable: caller is not the owner");
require(minters[msg.sender] || msg.sender == owner(), "...");
```

### Solana Anchor
```rust
// Owner check in account constraints:
#[account(constraint = authority.key() == token_state.owner @ LPTokenError::Unauthorized)]

// Or runtime check in apply():
require!(
    ctx.accounts.authority.key() == ctx.accounts.token_state.owner,
    LPTokenError::Unauthorized
);
```

### Analysis
- Anchor's `Signer<'info>` ensures the account has signed the transaction before the instruction executes
- The signature is verified by the Solana runtime before any program logic runs
- Equivalent to EVM's `msg.sender` with the added guarantee that the signature was checked cryptographically

---

## 2. Owner Access Control: `onlyOwner` → owner pubkey comparison

### EVM
```solidity
modifier onlyOwner() {
    require(msg.sender == owner(), "Ownable: caller is not the owner");
    _;
}
```

### Solana Anchor
```rust
// In instruction handler apply():
require!(
    ctx.accounts.owner.key() == ctx.accounts.token_state.owner,
    LPTokenError::Unauthorized
);
```

### Threat: Account Substitution
- **EVM:** The owner address is stored in contract storage; attackers cannot substitute it
- **Solana Risk:** A malicious client could pass a different `token_state` account if the program doesn't verify seeds
- **Mitigation:** `token_state` is a PDA with strict seeds `[b"token_state", mint.key()]`. Anchor verifies the PDA derivation, preventing account substitution.

---

## 3. Minter Access Control: `onlyMintersOrOwner` → PDA existence check

### EVM
```solidity
mapping(address => bool) public minters;
modifier onlyMintersOrOwner() {
    require(minters[msg.sender] || msg.sender == owner(), "...");
    _;
}
```

### Solana Anchor
```rust
// In mint_tokens / burn_tokens apply():
let is_owner = ctx.accounts.authority.key() == ctx.accounts.token_state.owner;
if !is_owner {
    // Verify PDA address matches (prevents passing a fake MinterRecord)
    let (expected_pda, _) = Pubkey::find_program_address(
        &[b"minter", token_state.key().as_ref(), authority.key().as_ref()],
        ctx.program_id,
    );
    require!(minter_record.key() == &expected_pda, LPTokenError::Unauthorized);

    // Deserialize and check is_active
    let data = minter_record.try_borrow_data()?;
    let record = MinterRecord::try_deserialize(&mut &data[..])?;
    require!(record.is_active, LPTokenError::Unauthorized);
}
```

### Threat: Fake MinterRecord Account
- An attacker could craft an account at any address and claim it's a MinterRecord
- **Mitigation:** The PDA address is deterministically derived from `[b"minter", token_state_key, authority_key]`. The program verifies the passed account matches this derivation before trusting its contents.
- The Anchor discriminator check in `try_deserialize` ensures the account was created by this program (not an attacker-crafted account).

---

## 4. Pause Guard: `whenNotPaused` → PDA field constraint

### EVM
```solidity
modifier whenNotPaused() {
    require(!paused(), "Pausable: paused");
    _;
}
```

### Solana Anchor
```rust
// Account constraint:
#[account(
    seeds = [b"token_state", token_mint.key().as_ref()],
    bump = token_state.bump,
    constraint = !token_state.is_paused @ LPTokenError::Paused,
)]
pub token_state: Account<'info, TokenState>,
```

### Analysis
- The constraint is checked before any instruction logic executes
- An attacker cannot bypass this by passing a different token_state because the PDA seeds are deterministic
- The `is_paused` field can only be modified by `set_pause` which requires the owner signature

---

## 5. Reentrancy: `nonReentrant` → Solana Architecture

### EVM
```solidity
modifier nonReentrant() {
    require(_status != _ENTERED, "ReentrancyGuard: reentrant call");
    _status = _ENTERED;
    _;
    _status = _NOT_ENTERED;
}
```

### Solana
- Solana's programming model does not allow a program to call itself recursively in the same transaction by default
- Cross-Program Invocations (CPIs) to the SPL Token program do not re-enter the `lp_token` program
- The runtime enforces that each account can only be mutably borrowed once per instruction
- **Conclusion:** No explicit reentrancy guard is needed

---

## 6. Mint Authority Protection

### EVM
The contract itself IS the token; there is no separate "mint authority" concept. `_mint()` is an internal function called only from `mint()` which has access control guards.

### Solana
- The SPL mint's `mint_authority` is set to the `token_state` PDA at initialization
- Only the program can sign as `token_state` (via PDA signing with seeds)
- No external key has direct mint authority — it is program-controlled
- **If the program is compromised, all mint authority is compromised** — this mirrors the EVM model exactly

### Threat: mint_authority Takeover
- Solana allows `setAuthority` to transfer mint authority permanently
- **Mitigation:** No `transfer_mint_authority` instruction is exposed. The program does not call `setAuthority`. The mint_authority remains the PDA permanently unless a new instruction is explicitly added in a future upgrade.

---

## 7. Account Validation: Constraint Completeness

Every instruction that accepts a `token_mint` account validates it matches the `token_state`:

```rust
#[account(
    constraint = token_account.mint == token_mint.key() @ LPTokenError::InvalidMint,
)]
pub token_account: Account<'info, TokenAccount>,
```

Every instruction validates the `token_state` PDA is for the correct mint:
```rust
#[account(
    seeds = [b"token_state", token_mint.key().as_ref()],
    bump = token_state.bump,
)]
pub token_state: Account<'info, TokenState>,
```

### Threat: Account Confusion
- Passing a valid `TokenState` for a different mint to gain elevated privileges
- **Mitigation:** The `token_state` seeds include `token_mint.key()`, binding them together. Substituting one without the other will cause PDA derivation to fail.

---

## 8. Burn Authorization Security

### EVM Behavior (Insecure by Solana Standards)
```solidity
function burn(address _account, uint256 _amount) external onlyMintersOrOwner {
    _burn(_account, _amount);  // Burns from ANY address, no consent required
}
```

### Solana Implementation (Safer)
```rust
// Both the minter authority AND the token account owner must sign
pub authority: Signer<'info>,           // minter/owner role
pub token_account_authority: Signer<'info>,  // token account holder

// Constraint enforcing the holder matches
#[account(constraint = token_account.owner == token_account_authority.key())]
```

### Analysis
- Requiring the token account holder to co-sign is a **security improvement** over EVM
- In practice, the LP bond use case always involves the user signing (they initiate the redemption transaction)
- This prevents rogue minter contracts from draining user balances without user interaction

---

## 9. CPI Safety

### SPL Token CPI calls are made with:
1. **Verified accounts** — all account addresses are validated before the CPI
2. **PDA signing** — mint_to uses `token_state` PDA seeds; no private key is ever exposed
3. **Correct program ID** — `ctx.accounts.token_program` is typed as `Program<'info, Token>`, so Anchor verifies it IS the SPL Token Program
4. **No arbitrary CPI** — the program only calls the SPL Token Program, never arbitrary programs

---

## 10. Upgrade Security

### EVM
- Upgrades go through ProxyAdmin (owner-controlled)
- The ProxyAdmin key is the ultimate security boundary

### Solana
- Programs are deployed with `BPFLoaderUpgradeable`
- The upgrade authority is a keypair set at deployment
- **Recommendation:** After final deployment, transfer upgrade authority to a multisig (Squads Protocol) or set to `None` to make the program immutable

---

## 11. Duplicate Operation Prevention

### EVM
```solidity
require(minters[_account] != _isMinter, "GMIToken: Duplicate operation");
```

### Solana
```rust
require!(
    ctx.accounts.minter_record.is_active != params.is_active,
    LPTokenError::DuplicateOperation
);
```

Identical semantic — the current value must differ from the requested value.

---

## Security Privilege Summary

| Privilege | EVM Holder | Solana Holder | Threat if Compromised |
|-----------|-----------|---------------|----------------------|
| Owner key | owner address | `token_state.owner` pubkey | Can pause, add rogue minters |
| Mint authority | implicit (contract) | `token_state` PDA (program-controlled) | Can mint unlimited tokens |
| Minter role | address in mapping | `MinterRecord` PDA is_active=true | Can mint to arbitrary accounts, burn with user consent |
| Upgrade authority | ProxyAdmin key | BPFLoader upgrade authority | Can replace entire program logic |

---

## Checklist

| Security Property | Status |
|-------------------|--------|
| Signer verification | ✓ Anchor `Signer<'info>` |
| Owner-only operations | ✓ pubkey comparison against `token_state.owner` |
| Minter role gating | ✓ MinterRecord PDA with discriminator check |
| Account substitution prevention | ✓ PDA seeds bind token_state to mint |
| Mint authority isolation | ✓ token_state PDA — no external key |
| Pause enforcement on mint/burn | ✓ constraint on token_state.is_paused |
| Reentrancy protection | ✓ Solana architecture (not needed explicitly) |
| CPI program validation | ✓ Anchor typed Program<'info, Token> |
| Burn consent | ✓ token account owner must co-sign (improvement over EVM) |
| Duplicate operation guard | ✓ is_active != new value check |
