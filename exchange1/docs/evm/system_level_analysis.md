# System-Level Analysis -- Energi LP Bonds Protocol

## System Architecture

The Energi LP Bonds protocol is a system of upgradeable smart contracts that creates a bond abstraction over Uniswap V3 liquidity positions. The system consists of:

```
                                    Off-chain Signer
                                          |
                                    (signature)
                                          |
    User --> LiquidityBondLockerV3 --> Uniswap V3 Position Manager
                    |                         |
                    |                    (LP NFT)
                    |                         |
                    |                    Multi-Sig Wallet
                    |
                    +--> LiquidityBonds (ERC721) --> User (Bond NFT)
                                |
                    (evolution) |
                                v
                    LiquidityBondsEvolution --> Uniswap V3 PM
                                |                    |
                                |               Multi-Sig
                                |
                                +--> LiquidityBonds (ERC721) --> User (New Bond NFT)
                                |
                    (exchange)  |
                                v
                    LPBondsExchange --> IERC20MintBurn --> User (ERC20 Tokens)
```

Each core contract is deployed behind a TransparentUpgradeableProxy.

---

## System-Level Features

### 1. Bond Minting Flow (Primary User Flow)

**Contracts involved**: LiquidityBondLockerV3, LiquidityBonds, Uniswap V3 Position Manager

**Flow**:
1. Off-chain service computes valid position parameters and signs them
2. User calls `LiquidityBondLockerV3.lockPositionChild()` with tokens + signature
3. Locker transfers user's token0 and token1 (or wraps ETH to WETH) to itself
4. Locker creates Uniswap V3 position via Position Manager
5. LP NFT is transferred to multisig wallet
6. Bond NFT is minted to user via `LiquidityBonds.mint()`
7. Lock record created in locker mapping

**Key invariants**:
- Each LP position NFT gets exactly one Bond NFT
- LP NFTs end up in the multisig, never with the user
- Bond NFT is the user's proof of locked liquidity
- User tokens are consumed by the Uniswap V3 position (minus rounding dust)

**Trust assumptions**:
- Off-chain signer is honest and provides correct parameters
- Multisig honestly custodies LP NFTs
- Uniswap V3 Position Manager is uncompromised

---

### 2. Bond Evolution Flow (Upgrade Path)

**Contracts involved**: LiquidityBondsEvolution, LiquidityBonds, IERC20MintBurn

**Flow**:
1. User owns a base-layer bond NFT and wants to evolve to a higher tier
2. User approves LiquidityBondsEvolution for their base-layer NFTs
3. Off-chain service signs evolution parameters
4. User calls `LiquidityBondsEvolution.lockPositionChild()` with base token IDs + token0 + signature
5. Base-layer NFTs are transferred to `multiSigBurned` (effectively burned)
6. Token0 is transferred from user to contract (plus fee to multisig)
7. Token1 is **minted** (not transferred) by the contract
8. New Uniswap V3 position created from token0 + minted token1
9. New LP NFT transferred to multisig
10. New (higher-tier) Bond NFT minted to user

**Key invariants**:
- Each evolution consumes exactly N base-layer bond NFTs to produce N new bond NFTs
- Token1 is inflated (minted) -- this is a deliberate monetary policy decision
- Fee is collected in token0 and sent to multisig
- Base-layer NFTs are not truly burned (sent to multiSigBurned address, not address(0))

**Trust assumptions**:
- Evolution contract has MINTER role on token1
- Off-chain signer validates correct token amounts for the evolution tier
- `multiSigBurned` address securely holds "burned" NFTs

---

### 3. Bond Exchange Flow (Redemption)

**Contracts involved**: LPBondsExchange, IERC20MintBurn

**Flow**:
1. User decides to exchange bond NFTs for ERC20 tokens
2. Off-chain service signs exchange parameters (per-NFT token amount)
3. User calls `LPBondsExchange.exchange()` with NFT token IDs + signature
4. Bond NFTs transferred from user to multisig
5. ERC20 tokens minted to user based on `_amount1 * tokenIds.length`

**Key invariants**:
- Each NFT exchanged yields `_amount1` tokens (exchange rate is off-chain determined)
- Exchange contract has MINTER role on the output ERC20 token
- NFTs are not burned, they go to multisig

**Trust assumptions**:
- Off-chain signer sets fair exchange rates
- Exchange token is an inflationary/mintable ERC20 controlled by the protocol

---

### 4. Liquidity Custody Model

**Who holds what**:
| Asset | Holder | How it got there |
|---|---|---|
| Uniswap V3 LP NFTs | `multiSig` wallet | Transferred from Locker/Evolution after minting |
| Bond NFTs | Users | Minted by LiquidityBonds to user |
| "Burned" base NFTs | `multiSigBurned` wallet | Transferred during evolution |
| Exchanged bond NFTs | `multiSig` wallet | Transferred during exchange |
| User's deposited tokens | Uniswap V3 Pool (via LP position) | Consumed during LP position minting |
| Residual tokens (rounding) | Locker/Evolution contract | Left over from LP mint; recoverable by owner |

**Critical observation**: The multisig wallet is the ultimate custodian of all LP positions. Users do NOT hold their LP positions -- they hold derivative NFTs (bonds). The multisig could theoretically:
- Remove liquidity from the positions
- Collect accumulated fees
- Transfer positions to other addresses

This is a **fully custodial** model with respect to the underlying liquidity. Users must trust the multisig operators.

---

### 5. Signature Verification Pipeline

All three core contracts use the same signature scheme:

```
hash = keccak256(abi.encodePacked(basePosition, amount0, amount1, contractAddress, nonce, msgSender))
ethSignedHash = toEthSignedMessageHash(hash)
recoveredSigner = ECDSA.recover(ethSignedHash, signature)
require(recoveredSigner == signer)
```

**Shared vulnerabilities across all contracts**:

1. **No chain ID**: Signatures are valid across any chain where contracts are deployed at the same address with matching nonce
2. **Global nonce**: Creates ordering dependencies and race conditions between users
3. **Missing parameters**: Key parameters are not bound:
   - Locker: `_numberOfBonds`, `_isEth` not signed
   - Evolution: `_numberOfBonds`, `_layerId`, `_fee` not signed
   - Exchange: `tokenIds`/count not signed
4. **`abi.encodePacked` collision risk**: While currently safe (all fixed-size types), any future addition of dynamic types would introduce hash collision vulnerabilities

---

### 6. Admin Privilege Scope

The owner role across all contracts has extensive powers:

| Capability | Contract | Impact |
|---|---|---|
| Upgrade implementation | ProxyAdmin (all) | Complete code replacement; can steal all assets |
| Set bond configuration | Locker, Evolution | Defines token pairs, tick ranges, fees |
| Set signer | Locker, Evolution | Controls who can authorize position parameters |
| Set multiSig | All three | Controls custody of all LP NFTs and fees |
| Pause/unpause | All three | Can freeze all user operations |
| Add/remove minters | LiquidityBonds | Controls who can mint/burn bond NFTs |
| Recover any ERC20/ETH/ERC721 | Locker, Evolution | Can extract any assets from contract |
| Set weird ERC20 flags | Locker | Changes token interaction behavior |
| Change operator registry | LiquidityBonds | Changes transfer restriction policy |

**Summary**: The owner has god-mode access. There are no timelocks, multi-sig requirements (on-chain), or governance checks on any admin action.

---

### 7. Cross-Contract Dependencies

```
LiquidityBonds
  --> ILiquidityBondLocker (reads lock data, rewards)
  --> IOperatorRegistry (transfer/approval validation)
  --> INonFungiblePositionManager (position data for tokenURI)

LiquidityBondLockerV3
  --> ILiquidityBonds (mints bond NFTs)
  --> INonFungiblePositionManager (creates LP positions)
  --> IWETH (wraps ETH)
  --> IERC20 / IERC20Weird (token transfers)

LiquidityBondsEvolution
  --> ILiquidityBonds (mints evolved bond NFTs)
  --> INonFungiblePositionManager (creates LP positions)
  --> IERC20MintBurn (mints token1, transfers token0)
  --> IERC721 (burns base-layer NFTs)

LPBondsExchange
  --> IERC721 (transfers bond NFTs to multisig)
  --> IERC20MintBurn (mints exchange tokens)
```

**Circular dependency**: LiquidityBonds reads from the Locker (for `getBondInfo`/`tokenURI`), and the Locker writes to LiquidityBonds (mints bonds). Both must reference each other's addresses.

---

### 8. Economic Invariants

1. **Bond supply == locked LP positions**: Every minted bond should correspond to exactly one Uniswap V3 LP position in the multisig. However, there is no on-chain enforcement -- the mapping is implicit via the `locks` mapping.

2. **Token1 inflation is controlled by evolution**: The Evolution contract mints token1 for new positions. The total token1 supply increases with each evolution. The only constraint is the off-chain signer's willingness to sign.

3. **No unlock mechanism exists on-chain**: There is no `unlockPosition` function in either locker contract (the interface declares one but it is never implemented). Once liquidity is locked, there is no on-chain way to retrieve it. Recovery depends entirely on the multisig.

4. **Rewards are fully off-chain**: `getRewards0()` returns 0 in both locker contracts. All reward calculations and distributions happen off-chain.

---

## Consolidated Findings Summary

### Critical (4)
| ID | Finding | Contract(s) |
|---|---|---|
| C-01 | `multiSig` / `multiSigBurned` can be set to `address(0)` with no validation | Locker, Evolution, Exchange |
| C-02 | Unlimited ERC20 approvals granted to Position Manager on every call | Locker, Evolution |
| C-03 | Evolution contract can mint unlimited token1 (constrained only by off-chain signer) | Evolution |
| C-04 | Any minter can burn any user's bond NFT without holder consent | LiquidityBonds |

### High (11)
| ID | Finding | Contract(s) |
|---|---|---|
| H-01 | Global nonce creates race conditions between concurrent users | All three |
| H-02 | Signature does not bind critical parameters (`_numberOfBonds`, `_isEth`, `_layerId`, `tokenIds`) | All three |
| H-03 | No chain ID in signature -- cross-chain replay possible | All three |
| H-04 | `tx.origin` usage for EOA detection breaks smart contract wallets | LiquidityBonds |
| H-05 | ProxyAdmin single-owner with no timelock enables instant malicious upgrades | All proxies |
| H-06 | Changing `uniswapPositionManager` exposes all approved token balances | Locker, Evolution |
| H-07 | Excess ETH not refunded to user | Locker |
| H-08 | `bondExists` modifier validates wrong bond in Evolution (checks `_bondId` not `layer.bondId`) | Evolution |
| H-09 | `setLayer` has zero input validation | Evolution |
| H-10 | `MultisigSet` event declared but never emitted | Exchange |
| H-11 | Signature interchangeable between collections with same base position | Exchange |

### Medium (10)
| ID | Finding | Contract(s) |
|---|---|---|
| M-01 | No zero-address validation in multiple `initialize()` functions | LiquidityBonds, Exchange |
| M-02 | `extcodesize` bypass during contract construction | LiquidityBonds |
| M-03 | No uniqueness check on `uniswapV3PositionId` across bonds | LiquidityBonds |
| M-04 | Locker update does not sync minter roles | LiquidityBonds |
| M-05 | `setBond` allows zero slippage protection (`amount0Min/amount1Min = 0`) | Locker, Evolution |
| M-06 | Weird ERC20 handling only for token1, not token0 | Locker |
| M-07 | `payable` functions without ETH handling trap sent ETH | Evolution, Exchange |
| M-08 | `recoverERC20` does not check transfer return value | Locker, Evolution |
| M-09 | No asset recovery functions in Exchange contract | Exchange |
| M-10 | `outputLayer` and `token` in Layer struct are dead storage (never read) | Evolution |

### Low / Informational (12)
| ID | Finding | Contract(s) |
|---|---|---|
| L-01 | Typo: `isRedemeed` should be `isRedeemed` | LiquidityBonds |
| L-02 | `lockDuration` naming confusion (treated as absolute timestamp, not relative duration) | LiquidityBonds |
| L-03 | `getRewards0` always returns 0 (dead code) | Locker, Evolution |
| L-04 | Multiple admin functions emit no events | All |
| L-05 | `this.symbol()` unnecessary external self-call | LiquidityBonds |
| L-06 | Fee can round to zero with small amounts | Evolution |
| L-07 | No `setSigner` function in Exchange (signer is immutable post-init) | Exchange |
| L-08 | `BondConfigSet` event missing `_isActive` field | Exchange |
| L-09 | `_amount0` signed but unused in Exchange | Exchange |
| L-10 | Bond struct persists after burn with stale data | LiquidityBonds |
| L-11 | No on-chain unlock mechanism despite interface declaring one | Locker |
| L-12 | Storage layout must be manually maintained across upgrades | All proxied contracts |

---

## Recommendations for Solana Port

When porting this system to Solana, the following EVM-specific patterns need Solana-native equivalents:

1. **Proxy upgradeability** -> Solana program upgrades via upgrade authority
2. **ERC721 NFTs** -> SPL Token-2022 or Metaplex NFTs
3. **Uniswap V3 positions** -> Orca Whirlpool positions (already in the Solana codebase)
4. **ECDSA signature verification** -> Ed25519 signature verification (native on Solana)
5. **`tx.origin` checks** -> No equivalent; use PDA-based authorization instead
6. **ERC20 approve/transferFrom pattern** -> SPL token delegate or direct CPI transfers
7. **Operator registry whitelist** -> Can be implemented as a PDA lookup or removed entirely
8. **Pausable pattern** -> Store a paused flag in a program state account
9. **Reentrancy guard** -> Solana's runtime provides reentrancy protection natively
10. **Global nonce** -> Consider per-user nonces or Solana's transaction uniqueness guarantees

### Critical behavioral differences to preserve:
- Bond-to-LP-position 1:1 mapping
- Custody model (LP positions held by protocol, bonds held by users)
- Signature-based parameter validation
- Layer/evolution tier system
- Fee collection mechanism
- Admin privilege scope and separation
