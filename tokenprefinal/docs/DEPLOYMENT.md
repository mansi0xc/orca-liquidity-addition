# Deployment Guide — LP Token (Solana)

## Program ID

Before deployment, generate a real program keypair:

```bash
solana-keygen new -o target/deploy/lp_token-keypair.json
```

Copy the resulting public key into `declare_id!()` in `programs/lp_token/src/lib.rs`, then rebuild.

---

## Upgrade Authority

### Background

On EVM, upgradability is managed via a ProxyAdmin contract that controls which implementation a proxy points to. On Solana, the **upgrade authority** is a native concept — the account authorized to deploy new bytecode to a program address.

By default, the deployer's keypair is the upgrade authority.

### Setting Upgrade Authority to a Multisig

After deployment, transfer upgrade authority to a multisig (recommended: [Squads Protocol](https://squads.so)):

```bash
solana program set-upgrade-authority <PROGRAM_ID> \
  --new-upgrade-authority <MULTISIG_ADDRESS>
```

This ensures no single key can unilaterally upgrade the program, matching the security posture of EVM's ProxyAdmin controlled by a multisig.

### Recommended Setup

1. Deploy the program using a deployer keypair.
2. Create a Squads multisig with appropriate threshold (e.g., 3-of-5).
3. Transfer upgrade authority to the Squads multisig vault.
4. Verify with: `solana program show <PROGRAM_ID>` — confirm the upgrade authority matches the multisig.

### Finalizing (Locking Upgrades)

To make the program permanently immutable (no future upgrades possible):

```bash
solana program set-upgrade-authority <PROGRAM_ID> --final
```

**Warning**: This is irreversible. Only finalize after thorough testing and when you are certain no further upgrades will ever be needed.

---

## Token Metadata

After deploying and initializing the mint, call the `set_metadata` instruction to attach on-chain metadata (name, symbol, URI) via the Metaplex Token Metadata program. This is the Solana equivalent of setting `name()` and `symbol()` during EVM initialization.

---

## Checklist

- [ ] Generate production program keypair
- [ ] Update `declare_id!()` with real program ID
- [ ] Deploy to devnet and run full test suite
- [ ] Deploy to mainnet
- [ ] Transfer upgrade authority to Squads multisig
- [ ] Call `initialize_mint` with correct owner and parameters
- [ ] Call `set_metadata` to set token name, symbol, and URI
- [ ] Verify program state on-chain
