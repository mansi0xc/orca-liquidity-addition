# 06 — Architecture Differences: EVM vs Solana

---

## What Remained the Same

### 1. Economic Logic
The economic model is **preserved identically**:

- **Protocol fee**: Calculated as `amount * protocolFeeBps / 10000`, paid by the NFT seller. Same formula, same basis point arithmetic.
- **Royalties**: Same cascading lookup hierarchy (owner token-level → owner collection-level → creator token-level → external provider → on-chain metadata). Same 50% cap enforcement.
- **Origin fees**: Same treatment — taker-side origin fees added on top of order amount, maker-side origin fees subtracted from maker payouts.
- **Payouts**: Same 100% sum validation and distribution logic. Last recipient gets the remainder to avoid rounding issues.
- **Fee side determination**: Identical priority chain: SOL (ETH) → wSOL (WETH) → SPL Token (ERC-20) → Semi-fungible (ERC-1155) → None.

### 2. Protocol Workflow
The high-level workflow is **identical**:

1. Off-chain order creation and signing
2. Off-chain Order Book validation and matchAllowance issuance
3. On-chain order matching with signature verification
4. On-chain fill computation and partial order support
5. On-chain fee distribution and asset transfer
6. Order cancellation by setting fill to MAX

### 3. Token Flows
Asset transfer logic is **functionally identical**:

- Fungible assets (SOL/wSOL/SPL) flow from buyer to seller (minus fees/royalties).
- Non-fungible assets (NFTs/SFTs) flow from seller to buyer.
- Only fungible-for-non-fungible trades are allowed.
- Whitelisted token enforcement is preserved.

### 4. Security Model
Core security properties are **preserved**:

- Order signatures prevent unauthorized order execution.
- Order Book matchAllowance provides time-limited matching authorization.
- Fill tracking prevents double-spending of orders.
- Monotonically increasing fills ensure order integrity.
- Order cancellation is permanent (fill = MAX).
- Pausing halts all operations.
- Protocol fee, royalty, and payout calculations are deterministic and verifiable.
- Role separation (owner, exchange_owner, order_book, upgrade_authority) is maintained.

---

## What Changed Due to Solana Architecture

### 1. Storage Model

**EVM:**
- Contract state lives in storage slots within the contract's account.
- Mappings (`mapping(bytes32 => uint256)`) are accessed via `SLOAD`/`SSTORE` by key hash.
- Separate storage contracts (`ExchangeStorage`, `RoyaltiesRegistryStorage`) hold state, accessed via external calls.
- No upfront allocation — storage grows dynamically.

**Solana:**
- State lives in **separate accounts** owned by the program (PDAs).
- Each "mapping entry" is a separate on-chain account with a deterministic address (PDA).
- Accounts must be **pre-allocated with sufficient size** and rent-exempt SOL.
- All accounts accessed by an instruction must be **declared upfront** in the transaction.

**Why this changed:** Solana's account model requires explicit account declaration at transaction time for parallel execution. Unlike EVM where a contract can access any storage slot, a Solana program can only read/write accounts listed in the instruction. This fundamentally changes how state is organized — from implicit storage slots to explicit PDA accounts.

**Impact:**
- `fills[orderKeyHash]` → `OrderFill` PDA per order key hash (account creation costs ~0.002 SOL each)
- `allowedERC20Assets[addr]` → `AllowedToken` PDA per token mint
- `feeReceivers[token]` → `FeeReceiver` PDA per token mint
- Royalty data → separate PDAs per collection/token combination
- All these accounts must be passed to the instruction, increasing transaction size

---

### 2. Transaction Structure

**EVM:**
- A single transaction calls one function, which can make unlimited internal calls and state accesses.
- Transaction size is essentially unlimited (only bounded by block gas limit).
- Batch operations (`batchMatchOrders`) can handle many pairs in one tx.
- Gas limit is the primary constraint (~30M gas per block).

**Solana:**
- Transaction size limit: **1232 bytes** (legacy) or larger with versioned transactions + address lookup tables.
- Compute budget: **200,000 CU default**, extendable to **1,400,000 CU** per transaction.
- All accounts must be declared in the transaction, consuming space.
- Each account address = 32 bytes in the transaction.

**Why this changed:** Solana's architecture optimizes for parallel execution across validators. Fixed transaction sizes and upfront account declaration enable the runtime to determine which transactions can execute in parallel. This trades single-transaction throughput for network-wide parallelism.

**Impact:**
- `match_orders` with many royalty recipients and payout addresses may require **address lookup tables** (versioned transactions) to fit all accounts.
- `batchMatchOrders` is severely limited — likely 1-2 pairs per transaction max. Multiple transactions in a bundle replace single-tx batching.
- Complex collection bid matching may need to be split across transactions.
- Compute budget extension (`set_compute_unit_limit`) will be needed for `match_orders`.

---

### 3. Signature Verification

**EVM:**
- Uses **secp256k1** ECDSA signatures.
- **EIP-712** typed data hashing with domain separator.
- `ecrecover` opcode recovers signer address from signature.
- **EIP-1271** for smart contract signature verification.
- Costs ~3,000 gas per ecrecover.

**Solana:**
- Native signatures use **Ed25519** curve.
- Signature verification via **Ed25519SigVerify program** (sysvar introspection) or **Secp256k1SigVerify program**.
- No `ecrecover` equivalent — verification is done via precompile instruction introspection.
- The verifying instruction must precede the program instruction in the same transaction.

**Why this changed:** Solana uses Ed25519 as its native signature scheme (faster, more secure). Secp256k1 support exists but as a precompile, not a native opcode. The verification model is "precompile + introspection" rather than "inline opcode."

**Impact:**
- **Option A (Ed25519):** New signature format. Off-chain Order Book and users must sign with Ed25519 keys. Faster, cheaper, idiomatic.
- **Option B (Secp256k1):** Preserves exact EVM signatures. The `Secp256k1SigVerify` instruction is added before `match_orders` in the same transaction. The program introspects the previous instruction's data to confirm verification passed. Higher CU cost (~100k CU).
- EIP-1271 (smart contract wallet signatures) has no direct equivalent. Could be replicated via CPI to a verification program but is rare on Solana.
- EIP-712 domain separator concept → custom domain bytes (`program_id + "energi" + version`).

---

### 4. Event Handling

**EVM:**
- Events (`emit Match(...)`) are logged in transaction receipts.
- Events are indexed and queryable via `eth_getLogs`.
- Events are cheap (375 gas + 8 gas/byte for indexed topics).
- Events have indexed parameters for efficient filtering.

**Solana:**
- No native event system.
- **Anchor events** (`emit!()`) write serialized data to transaction logs.
- Logs are queryable via `getTransaction` RPC call but NOT efficiently indexed on-chain.
- Off-chain indexers (e.g., Geyser plugins, Helius, Shyft) are used for event streaming.
- Account state changes are the primary "event" mechanism (polling account data).

**Why this changed:** Solana optimizes for throughput and parallel execution. Persistent indexed event logs would add storage overhead. Instead, Solana relies on off-chain infrastructure for indexing.

**Impact:**
- `Match`, `CancelOrder`, and `Transfer` events are emitted via `emit!()` in Anchor.
- An off-chain indexer must subscribe to program logs to capture events.
- For durable state queries, the `OrderFill` PDA accounts serve as permanent records.
- The `Transfer` event is less critical because Solana's token program emits its own transfer logs.

---

### 5. Account Validation

**EVM:**
- Minimal account validation needed — addresses are 20-byte values, no "account existence" concept.
- Token contracts validate balances and allowances internally.
- `msg.sender` and `tx.origin` are implicitly trusted.

**Solana:**
- Every account passed to an instruction must be **validated**:
  - Is it the correct PDA? (seeds + bump verification)
  - Is it owned by the expected program?
  - Is it the expected token account for the expected mint?
  - Is it writable if the instruction will modify it?
  - Is it a signer if signature is required?
- Anchor handles most validation via `#[derive(Accounts)]` constraints.

**Why this changed:** Solana's account model requires explicit validation because any program can be called with arbitrary accounts. Without validation, an attacker could substitute fake accounts.

**Impact:**
- Extensive account validation logic in the `#[derive(Accounts)]` struct.
- PDA seeds must be verified for every PDA account.
- Token account ownership and mint must be verified.
- This is additional code but provides stronger guarantees than EVM.

---

### 6. Concurrency Model

**EVM:**
- Transactions execute **sequentially** within a block.
- Reentrancy is possible (mitigated by `nonReentrant` guard).
- No parallel execution within a block.

**Solana:**
- Transactions can execute **in parallel** if they don't share accounts.
- A program is **locked** during execution — no reentrancy is possible.
- Transactions accessing the same accounts are serialized by the runtime.
- Optimistic concurrency — if two transactions conflict, one succeeds and the other is retried.

**Why this changed:** Solana achieves high throughput via parallel execution. The account-based locking mechanism prevents conflicts without manual reentrancy guards.

**Impact:**
- `nonReentrant` modifier is **not needed** — Solana's runtime prevents reentrancy at the program level.
- Two `match_orders` transactions involving different order pairs (and different token accounts) can execute in parallel.
- Two transactions involving the **same** `OrderFill` PDA will be serialized, preventing double-fill.
- The `ExchangeConfig` account is read by every transaction — since it's read-only in most instructions, this doesn't cause serialization.

---

### 7. Proxy/Upgradeability Pattern

**EVM:**
- UUPS (ERC-1967) proxy pattern with `delegatecall`.
- Proxy holds state; implementation holds logic.
- Upgrade = deploy new implementation + call `upgradeToAndCall` on proxy.
- Users interact with proxy address (stable).
- `UpgradeManager` role authorizes upgrades.

**Solana:**
- Programs are **natively upgradeable** via BPF loader.
- The program's **upgrade authority** (a pubkey, can be a multisig) authorizes deploys.
- Program ID remains stable across upgrades.
- No proxy contracts needed.
- State accounts (PDAs) are separate from the program — they persist across upgrades.

**Why this changed:** Solana has native upgradeability built into the runtime. The complex proxy patterns from EVM are unnecessary.

**Impact:**
- `ExchangeProxy`, `ExchangeHelperProxy`, `RoyaltiesRegistryProxy` are **not needed**.
- `UpgradeManager` contract is replaced by setting the program's upgrade authority to a multisig (e.g., Squads multisig on Solana).
- Signature domains reference `program_id` instead of `proxy_address` (equivalent concept — stable address across upgrades).

---

### 8. ETH/WETH vs SOL/wSOL Handling

**EVM:**
- `msg.value` for ETH.
- WETH is a standard ERC-20 contract with `deposit()`/`withdraw()` functions.
- Complex ETH ↔ WETH conversion logic in `processEthAndWeth`.
- ETH is forwarded to proxy via `receiveETH()`.
- Protocol fees from WETH trades are unwrapped to ETH before forwarding.

**Solana:**
- SOL is handled via `system_program::transfer`.
- wSOL is an SPL token with a specific mint address. Wrapping = creating an ATA with native SOL + `sync_native`. Unwrapping = closing the wSOL ATA.
- The conversion logic is simpler because wSOL wrapping/unwrapping is more straightforward.

**Why this changed:** Solana's wSOL mechanism is different from EVM's WETH. On Solana, wrapping SOL is just funding a token account and calling `sync_native`, not calling a `deposit()` function on a contract.

**Impact:**
- `processEthAndWeth` is simplified to `process_sol_and_wsol`.
- No need to forward funds to a proxy — the program's PDA can hold SOL/wSOL temporarily during the instruction.
- Protocol fees can be paid in SOL by closing a wSOL temp account.
- The `PROXY_WETH_ASSET_CLASS` concept may be simplified or removed since there's no proxy holding wSOL.

---

### 9. Library vs Inline Code

**EVM:**
- Heavy use of Solidity libraries (`LibExchange`, `LibOrder`, `LibFill`, etc.) to keep contract sizes under 24KB.
- Libraries are deployed separately and linked, or inlined by the compiler.
- `ExchangeHelper` exists solely to circumvent the 24KB contract size limit.

**Solana:**
- No program size limit (well, ~10MB max, but practically unlimited for business logic).
- All "library" logic is **inline Rust functions/modules** within the same program.
- No need for a separate helper program.

**Why this changed:** Solana programs don't have the 24KB size limit that drove EVM's library separation pattern.

**Impact:**
- All library code (`LibExchange`, `LibOrder`, `LibFill`, `LibSignature`, `LibEIP712`, `LibMath`, `LibBps`, `LibFeeSide`, `LibOrderData`) becomes Rust modules within the exchange program.
- `ExchangeHelper` is fully merged into the exchange program.
- This simplifies the architecture significantly — one program instead of three interacting contracts.

---

### 10. Native Token Transfer Mechanism

**EVM:**
- ETH transfers: `payable(to).call{value: amount}('')`
- ERC-20 transfers: `IERC20(token).transferFrom(from, to, value)` (requires approval)
- ERC-721 transfers: `IERC721(token).safeTransferFrom(from, to, tokenId)` (requires approval)
- ERC-1155 transfers: `IERC1155(token).safeTransferFrom(from, to, id, value, '')` (requires approval)
- The Exchange contract calls these directly (or via proxy).

**Solana:**
- SOL transfers: `system_program::transfer(from, to, amount)` — requires `from` to be a signer
- SPL transfers: `spl_token::transfer(source_ata, dest_ata, authority, amount)` — requires authority to be a signer or delegate
- The program must invoke these via CPI (Cross-Program Invocation).
- Token accounts (ATAs) must exist before transfer.

**Why this changed:** Solana's token program is a separate program, not embedded in the runtime. All token operations are CPI calls. The "approval" model uses delegation rather than unlimited allowances.

**Impact:**
- Every transfer becomes a CPI call to `system_program` or `spl_token`.
- The maker/taker must either:
  - Be a transaction signer (for `salt == 0` orders), OR
  - Have delegated their token account to the exchange program's PDA (for `salt > 0` orders where maker is not present)
- Associated Token Accounts (ATAs) must be created if they don't exist (the exchange may need to create ATAs for recipients on-the-fly).
- NFT transfers are just SPL token transfers with amount=1.

---

## Summary Table

| Aspect | EVM | Solana | Change Type |
|---|---|---|---|
| Smart contract entry point | Proxy contract | Program directly | Simplified |
| State storage | Contract storage slots | PDA accounts | Restructured |
| Dynamic storage (mappings) | Hash-based slot access | PDA per entry | More explicit |
| Transaction size | Unlimited (gas-bounded) | 1232 bytes (expandable) | Constrained |
| Batch operations | Unlimited in one tx | Limited per tx | Split across txs |
| Signature scheme | secp256k1 / EIP-712 | Ed25519 (or secp256k1 via precompile) | Changed or adapted |
| Signature verification | `ecrecover` opcode | Precompile + introspection | Different mechanism |
| Event system | Native events + indexing | Program logs + off-chain indexers | Different infrastructure |
| Account validation | Minimal (addresses) | Extensive (PDA, ownership, mint) | More rigorous |
| Reentrancy protection | Manual (`nonReentrant`) | Runtime-enforced | Simplified |
| Upgradeability | UUPS proxy pattern | Native BPF upgrade | Simplified |
| Contract size limit | 24KB per contract | ~10MB per program | Relaxed |
| Token transfers | Direct calls | CPI to token program | Indirection added |
| Approval model | ERC-20 `approve` | SPL `delegate` or signer | Different mechanism |
| Native currency wrapping | WETH contract | wSOL (sync_native) | Different mechanism |
| Library separation | Required (size limit) | Optional (code organization) | Simplified |
