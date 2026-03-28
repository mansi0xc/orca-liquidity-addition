---
name: EVM LP Bonds Security Findings
description: Key security findings from the EVM contract audit that should be checked in the Solana port
type: project
---

Critical/High severity findings from the EVM audit (2026-03-25):

1. **multiSig/multiSigBurned zero-address risk** -- No validation allows LP NFTs to be burned
2. **Unlimited ERC20 approvals** -- type(uint256).max approvals to Position Manager on every call
3. **Global nonce race condition** -- Concurrent users compete for same nonce; DoS vector
4. **Signature parameter binding gaps** -- _numberOfBonds, _isEth, _layerId, tokenIds, chainId not signed
5. **Any minter can burn any bond** -- No holder consent check in burn()
6. **tx.origin for EOA detection** -- Breaks smart contract wallets
7. **bondExists modifier validates wrong bond in Evolution** -- Checks _bondId but uses layer.bondId
8. **Evolution mints unlimited token1** -- Only off-chain signer constrains inflation

**Why:** These findings represent real vulnerabilities that should NOT be replicated in the Solana port.

**How to apply:** When auditing Solana LP bond programs, verify each of these patterns is handled correctly. Particularly: signature binding, nonce design, access control for burns, and zero-address/zero-account validation.
