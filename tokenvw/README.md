# LP Token — Solana Migration

Solana Anchor program implementing the EVM `LPToken.sol` (Energi Core) as an SPL Token wrapper with access-controlled mint/burn operations.

## Architecture

The program wraps the **SPL Token Program** for all balance accounting and adds:
- **Minter role management** — owner can register/deregister minter addresses
- **Pause mechanism** — blocks mint/burn while allowing transfers (matches EVM behavior)
- **Owner governance** — single admin with exclusive control over minters, pause state, and ownership

### EVM to Solana Mapping

| EVM Concept | Solana Equivalent |
|-------------|-------------------|
| ERC20 balances | SPL Token accounts (ATAs) |
| `totalSupply` | SPL Mint `supply` field |
| `minters[addr]` | `MinterRecord` PDA per (mint, minter) |
| `owner` | `TokenState.owner` pubkey |
| `paused` | `TokenState.is_paused` bool |
| Decimals: 18 | Decimals: 9 (u64 constraint) |

### Key Design Decisions

1. **Decimals: 9** — SPL balances are `u64`; with 18 decimals max supply would be ~18 tokens. 9 decimals allows ~18.4B tokens.
2. **Burn co-sign** — Solana requires the token account holder to sign burns (security improvement over EVM's unrestricted minter-burns-anyone pattern).
3. **Transfers not pause-gated** — Matches EVM LPToken which does NOT override `_transfer`.
4. **No reentrancy guard** — Solana's architecture prevents reentrancy by design.

## Program Structure

```
programs/lp_token/src/
├── lib.rs                           # Program entry point, instruction dispatch
├── errors.rs                        # LPTokenError enum
├── events.rs                        # Anchor events (EVM event equivalents)
├── instructions.rs                  # Module declarations
├── instructions/
│   ├── initialize_mint.rs           # Create SPL mint + TokenState PDA
│   ├── mint_tokens.rs               # Mint tokens (minter/owner, pause guard)
│   ├── burn_tokens.rs               # Burn tokens (minter/owner + holder co-sign)
│   ├── update_minter.rs             # Register/deregister minters (owner only)
│   ├── set_pause.rs                 # Pause/unpause (owner only)
│   ├── transfer_ownership.rs        # Transfer admin ownership
│   ├── transfer_tokens.rs           # SPL transfer wrapper (no custom guards)
│   └── approve_delegate.rs          # SPL approve wrapper (no custom guards)
├── state.rs                         # Module declarations
└── state/
    ├── token_state.rs               # TokenState PDA (owner, is_paused, evm_chain_id)
    └── minter_record.rs             # MinterRecord PDA (is_active, minter)
```

## Prerequisites

- **Rust** 1.86.0+ (`rustup install 1.86.0`)
- **Solana CLI** 3.1+ (`agave-install update`)
- **Anchor** 0.31.1 (`avm install 0.31.1 && avm use 0.31.1`)
- **Node.js** 18+ and **Yarn** (`npm install -g yarn`)

## Setup

```bash
git clone <repo-url> && cd solana-token
yarn install
```

## Build

```bash
anchor build -p lp_token
```

The compiled program will be at `target/deploy/lp_token.so` with the IDL at `target/idl/lp_token.json`.

## Test

```bash
anchor test
```

Runs 35 tests covering:
- Initialization (owner, chainId, pause state, decimals, supply)
- Minting (owner, minter, unauthorized, paused, no cap)
- Burning (owner co-sign, minter co-sign, unauthorized, paused)
- Minter management (add, remove, duplicate, unauthorized, deregistered)
- Pause/unpause (state transitions, guards, unauthorized)
- Transfers (correct amounts, not blocked by pause)
- Delegated transfers (approve, delegate transfer, approve while paused)
- Ownership transfer (transfer, unauthorized, governance after transfer)
- Edge cases (insufficient balance, zero mint)

## Security Considerations

| Property | Implementation |
|----------|---------------|
| Signer verification | Anchor `Signer<'info>` on all privileged accounts |
| Owner-only operations | `constraint = owner.key() == token_state.owner` |
| Minter validation | PDA derivation + discriminator + `is_active` flag |
| Account substitution prevention | PDA seeds bind `token_state` to specific mint |
| Mint authority isolation | `token_state` PDA — no external key holds mint authority |
| Pause enforcement | Constraint on `is_paused` for mint/burn only |
| CPI program validation | `Program<'info, Token>` typed account |
| Burn consent | Dual signer: minter authority + token account owner |

### Production Deployment

After deploying to mainnet:
1. Generate a real program keypair: `solana-keygen new -o target/deploy/lp_token-keypair.json`
2. Update `declare_id!` in `lib.rs` and `Anchor.toml` with the generated pubkey
3. Deploy: `anchor deploy --program-name lp_token`
4. Transfer upgrade authority to a multisig (Squads Protocol) or make immutable

## Migration Analysis

Detailed documentation of the EVM-to-Solana migration is in `migration-analysis/`:

| Document | Contents |
|----------|----------|
| `erc20-discovery.md` | Contract inventory and dependency map |
| `erc20-specification.md` | Full behavioral spec of LPToken.sol |
| `erc20-solana-design.md` | Solana account and instruction design |
| `erc20-security.md` | EVM to Solana security model translation |
| `erc20-migration-summary.md` | Architecture comparison and deployment steps |
| `review-context-summary.md` | Audit context and assumptions |
| `codebase-audit.md` | Full audit findings and resolutions |
