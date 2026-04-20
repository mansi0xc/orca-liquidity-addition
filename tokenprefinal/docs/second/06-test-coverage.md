# Test Coverage — `tests/lp_token.ts`

Assessment of the Anchor test suite against the parity matrix. Test file has **2365 lines** and `describe` / `it` blocks broken down below (line references are to start of each `it`).

## Per-instruction coverage matrix

| Instruction | Happy path | Auth denial | Pause gate | Edge cases | Adversarial |
|---|---|---|---|---|---|
| `initialize_mint` | 184-213 (5 tests: owner, chain_id, unpaused, decimals, initial supply) | ATK-14 (1854) re-init | N/A | 1389 zero-address owner | ATK-14 |
| `mint_tokens` | 216-278 (owner + minter) | 280 stranger; 642 deregistered; ATK-1/2/3 (1429-1535) | 306, ATK-22 (2058) | 354 no-cap, 1367 zero-amount | ATK-1, ATK-2, ATK-3, ATK-6, ATK-8, ATK-22 |
| `burn_tokens` | 385-439 (owner + minter dual sig) | 441 stranger; ATK-4 (missing cosigner) 1537; ATK-21 (wrong authority) 2028 | 468, ATK-23 (2104) | 1311 > balance; ATK-15 (1881) zero-amount | ATK-4, ATK-5, ATK-9, ATK-21, ATK-23 |
| `update_minter` | 535-553 add; 554-586 remove | 622 non-owner; ATK-12, ATK-13 (1798-1852) | N/A (no pause gate on updateMinter) | 587 duplicate-op | ATK-12, ATK-13 |
| `set_pause` (pause) | 696-720 | 803, ATK-10 (1764) | 749 (already-paused) | -- | ATK-10 |
| `set_pause` (unpause) | 722-748 | 803 | 786 (not-paused) | -- | -- |
| `transfer_ownership` | 1027-1082 | 1084, ATK-11 (1781) | N/A | 1101 zero-addr; 1162 not-yet-transferred; 2348 self as new owner (ATK-28) | ATK-11, ATK-25, ATK-28 |
| `accept_ownership` | 1027-1082 (combined with propose); 1202 new owner exercises governance | 1118 stranger; 1286 no-pending; ATK-26 (2225) | N/A | 2268 overwrite pending (ATK-27) | ATK-26, ATK-27 |
| `transfer_tokens` | 848-876 | 2008 non-delegate (ATK-20); 1344 > balance | 877 works when paused; ATK-24 (2151) | ATK-16 (1904) zero-amount | ATK-18 (> allowance), ATK-19 (revoked), ATK-20, ATK-24 |
| `approve_delegate` | 925-944; 945-977 delegate-can-transfer | 1965 revoked (ATK-19) | 978 works when paused | ATK-17 (1917) zero-amount | ATK-18, ATK-19 |

## Adversarial blocks (tests/lp_token.ts:1428-2365)

- **Unauthorized minting** (ATK-1..3): stranger passes others' PDA / random PDA / deactivated minter
- **Unauthorized burning** (ATK-4..5): missing co-signer, stranger burning own tokens (without minter/owner)
- **Cross-mint attacks** (ATK-6..9): wrong `token_state`/`token_mint`/`minter_record` combos across two mints
- **Authority bypass** (ATK-10..13): stranger pause / transferOwnership / register-minter / minter-registers-minter
- **Re-init** (ATK-14): second `initialize_mint` rejected
- **Zero-value** (ATK-15..17): burn / transfer / approve zero amounts succeed (ERC20 parity)
- **Delegate misuse** (ATK-18..20): over-limit / revoked / non-delegate
- **Burn authority mismatch** (ATK-21): minter specifies wrong `token_account_authority`
- **Pause bypass** (ATK-22..24): mint/burn during pause denied; transfer still works
- **Renunciation blocked** (ATK-25): cannot propose `Pubkey::default()`
- **Two-step attacks** (ATK-26..28): stranger-accept / overwrite-pending / self-as-new-owner

## Coverage gaps noted

The test suite is unusually thorough. A few items that could be added for completeness, in priority order:

1. **Mint-to-self allowance**: explicitly assert that the SPL `delegated_amount` decrements correctly after a delegate transfer (currently inferred from ATK-18 failing past limit). Low priority -- this is SPL behavior.
2. **Freeze / thaw**: `token_state` is set as `freeze_authority` (`initialize_mint.rs:36`). No test exercises freezing. This functionality is not part of EVM LPToken so it is not a parity gap, but if you do NOT intend to use freeze, consider setting `freeze_authority` to `None` to reduce privilege surface. Optional hardening.
3. **Concurrency / ordering**: no multi-transaction test asserting that a pause applied mid-batch correctly denies subsequent mints. Low priority -- the instruction-level test already proves the invariant.
4. **Rent exemption tear-down**: no test attempts to close / reclaim the `token_state` PDA. Expected behavior is that closing is impossible (no `close` instruction in the program). If you want to guarantee this, add a negative test that any attempt fails.
5. **Many-minter stress**: no test registers more than a handful of minters. Not a correctness concern; skip unless you want to surface CU measurements.

## Overall assessment

Coverage is at-or-above industry standard for a token program. All EVM require-statements have at least one negative-case test. All cross-account-substitution vectors are covered. The 28 ATK tests form a strong adversarial baseline.
