# Review Context Summary — LP Token Migration

**Date:** 2026-03-16
**Reviewer:** Automated audit pipeline

---

## ERC20 Feature Summary (LPToken.sol)

The migration target is `contracts/lp-token/LPToken.sol` — an ERC20 LP bond token by Energi Core.

| Feature | Details |
|---------|---------|
| Standard | ERC20Upgradeable (OpenZeppelin v4.5.0) |
| Decimals | 18 (EVM); mapped to 9 on Solana (u64 constraint) |
| Max Supply | None — unbounded minting |
| Access Control | Owner (Ownable) + Minter roles (custom mapping) |
| Pausability | Pause blocks mint/burn only; transfers and approvals are NOT blocked |
| Burn Model | Minter can burn from ANY address without consent |
| Upgradeability | TransparentUpgradeableProxy + ProxyAdmin |
| Reentrancy | ReentrancyGuardUpgradeable on mint/burn |

### Core Functions

1. **initialize** — one-time setup (name, symbol, owner, chainId)
2. **mint(address, uint256)** — onlyMintersOrOwner, whenNotPaused, nonReentrant
3. **burn(address, uint256)** — onlyMintersOrOwner, whenNotPaused, nonReentrant
4. **updateMinter(address, bool)** — onlyOwner, duplicate prevention
5. **pause() / unpause()** — onlyOwner, state transition guards
6. **transfer / transferFrom / approve** — standard ERC20, NO custom overrides
7. **impl()** — returns implementation address (proxy pattern)

---

## Required Solana Behaviors

| EVM Behavior | Solana Equivalent |
|-------------|-------------------|
| ERC20 balances | SPL Token accounts |
| totalSupply | SPL Mint supply field |
| owner role | TokenState.owner pubkey |
| minters mapping | MinterRecord PDA per (mint, minter) |
| paused flag | TokenState.is_paused |
| msg.sender check | Signer<'info> constraint |
| onlyOwner | pubkey comparison against token_state.owner |
| onlyMintersOrOwner | owner check OR MinterRecord PDA verification |
| whenNotPaused | constraint = !token_state.is_paused |
| nonReentrant | Solana architecture (implicit) |
| Proxy upgradeability | BPFLoaderUpgradeable (native) |

---

## Critical Security Constraints

1. **Signer verification** — all privileged operations require `Signer<'info>`
2. **PDA authority enforcement** — token_state PDA is mint_authority; seeds bind to specific mint
3. **Mint authority isolation** — no external key has direct mint authority
4. **Account substitution prevention** — PDA seeds include mint key; prevents cross-mint attacks
5. **Minter record validation** — PDA derivation + discriminator check + is_active flag
6. **Pause enforcement** — constraint on token_state.is_paused for mint/burn only
7. **CPI safety** — typed Program<'info, Token> ensures correct program ID
8. **Burn consent** — Solana improvement: token account owner must co-sign burns
9. **Duplicate operation guard** — is_active != requested value

---

## Known Assumptions

1. **Decimals change:** 18 → 9 is required by u64 balance constraints; cross-chain amounts must be scaled by 10^9
2. **Burn co-sign:** Solana requires token account owner signature; this is a security improvement over EVM's unrestricted minter burns
3. **No on-chain name/symbol:** SPL Token has no metadata fields; Metaplex Token Metadata program should be used post-init
4. **No max supply:** LPToken has no cap; the Solana program mirrors this
5. **Transfer/approve not pause-gated:** Matches EVM exactly — LPToken does NOT override _transfer or _approve
6. **Single owner model:** No multi-sig built in; recommend multisig via Squads Protocol for production
