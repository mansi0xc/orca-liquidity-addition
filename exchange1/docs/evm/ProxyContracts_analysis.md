# Proxy Contracts -- Functional Analysis

## Overview

The system uses OpenZeppelin's Transparent Proxy pattern for upgradeability. There are three proxy/admin pairs:

1. **LiquidityBondsProxy** + **LiquidityBondsProxyAdmin** -- proxy for `LiquidityBonds`
2. **LiquidityBondLockerProxy** + **LiquidityBondLockerProxyAdmin** -- proxy for `LiquidityBondLockerV3`
3. **LPBondsExchangeProxy** + **LPBondsExchangeProxyAdmin** -- proxy for `LPBondsExchange`

All six contracts are thin wrappers around OZ contracts with no custom logic.

---

## LiquidityBondsProxy

- **Inherits**: `TransparentUpgradeableProxy` (OpenZeppelin)
- **Purpose**: Delegates all calls to the `LiquidityBonds` implementation contract. Admin functions (upgrade, changeAdmin) are restricted to the ProxyAdmin contract.

### `constructor(address logic, address initialOwner, bytes data)`

| Section | Details |
|---|---|
| Purpose | Deploys the proxy, pointing to the `logic` implementation, with `initialOwner` as the admin, and optionally calling `data` on the implementation (typically `initialize()`). |
| Inputs | `logic`: implementation contract address; `initialOwner`: admin address (typically the ProxyAdmin contract); `data`: calldata for initialization |
| External Calls | Delegates to `logic` with `data` if non-empty |
| Security | Standard OZ pattern. The `initialOwner` should be the corresponding ProxyAdmin contract, NOT an EOA, to maintain the Transparent Proxy security model. |

---

## LiquidityBondsProxyAdmin

- **Inherits**: `ProxyAdmin` (OpenZeppelin)
- **Purpose**: Admin contract that can upgrade the proxy's implementation and change admin. Owned by a single address.

### `constructor(address initialOwner_)`

| Section | Details |
|---|---|
| Purpose | Deploys the ProxyAdmin with `initialOwner_` as the owner. |
| Inputs | `initialOwner_`: address that will own the ProxyAdmin (can upgrade, change admin) |
| Security | The owner of this contract has absolute power to upgrade the implementation, potentially replacing all contract logic with malicious code. This is the most privileged role in the system. Should be a multisig or governance contract. |

---

## LiquidityBondLockerProxy / LiquidityBondLockerProxyAdmin

Identical structure to LiquidityBondsProxy/ProxyAdmin. Same security considerations.

---

## LPBondsExchangeProxy / LPBondsExchangeProxyAdmin

Identical structure. Same security considerations.

---

## Proxy-Level Security Summary

### High Findings
1. **H-01: Single-owner ProxyAdmin** -- Each ProxyAdmin is owned by a single address. If this address is compromised, the attacker can upgrade any proxied contract to a malicious implementation, stealing all assets. The ProxyAdmin owner should be a multisig or governance mechanism with a timelock.

### Medium Findings
1. **M-01: No timelock on upgrades** -- Implementation upgrades take effect immediately. No delay for users to review and exit. A malicious upgrade can drain all assets instantly.
2. **M-02: Storage layout collision risk** -- Implementation upgrades must maintain storage layout compatibility. No on-chain enforcement exists. An incorrect upgrade could corrupt all state.

### Informational
1. **I-01: Standard OZ transparent proxy pattern** -- No custom logic means no custom bugs. Security relies entirely on correct deployment and admin key management.
2. **I-02: The `initialOwner` parameter in proxy constructor should be the ProxyAdmin contract address**, not an EOA, to prevent the admin from accidentally calling implementation functions through the proxy.
