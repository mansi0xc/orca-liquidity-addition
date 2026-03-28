/**
 * collectFees.ts — SDK helper for the collect_fees instruction.
 *
 * After Batch 1 fixes, collect_fees requires TWO additional accounts:
 *   - tick_array_lower: Tick array PDA for the position's lower tick
 *   - tick_array_upper: Tick array PDA for the position's upper tick
 *
 * These must be derived from:
 *   1. The position custody's tick_lower_index / tick_upper_index
 *   2. The whirlpool's tick_spacing
 *   3. The whirlpool address
 */

import { PublicKey } from "@solana/web3.js";

// =============================================================================
// CONSTANTS
// =============================================================================

const WHIRLPOOL_PROGRAM_ID = new PublicKey(
  "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
);

const TICK_ARRAY_SIZE = 88;

// =============================================================================
// TICK ARRAY DERIVATION
// =============================================================================

/**
 * Compute the start tick index for the tick array containing `tickIndex`.
 *
 * Mirrors whirlpool_cpi::get_start_tick_index in Rust.
 * Uses floor division for negative indices.
 *
 * @param tickIndex - The tick index (e.g., from PositionCustody.tick_lower_index)
 * @param tickSpacing - The whirlpool's tick spacing
 * @returns The start tick index for the containing tick array
 */
export function getStartTickIndex(
  tickIndex: number,
  tickSpacing: number
): number {
  const ticksInArray = TICK_ARRAY_SIZE * tickSpacing;
  let start = Math.trunc(tickIndex / ticksInArray);
  // JavaScript Math.trunc truncates toward zero (like Rust integer division).
  // For negative indices, we need floor division (toward -infinity).
  if (tickIndex < 0 && tickIndex % ticksInArray !== 0) {
    start -= 1;
  }
  return start * ticksInArray;
}

/**
 * Derive the tick array PDA for a given whirlpool and start_tick_index.
 *
 * Mirrors whirlpool_cpi::get_tick_array_address in Rust.
 *
 * @param whirlpool - The whirlpool public key
 * @param startTickIndex - The start tick index (from getStartTickIndex)
 * @returns [publicKey, bump] tuple
 */
export function getTickArrayAddress(
  whirlpool: PublicKey,
  startTickIndex: number
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("tick_array"),
      whirlpool.toBuffer(),
      Buffer.from(startTickIndex.toString()),
    ],
    WHIRLPOOL_PROGRAM_ID
  );
}

/**
 * Derive both tick array PDAs needed for the collect_fees instruction.
 *
 * @param whirlpool - The whirlpool public key
 * @param tickLowerIndex - The position's lower tick index
 * @param tickUpperIndex - The position's upper tick index
 * @param tickSpacing - The whirlpool's tick spacing
 * @returns Object with tickArrayLower and tickArrayUpper public keys
 */
export function deriveTickArraysForCollectFees(params: {
  whirlpool: PublicKey;
  tickLowerIndex: number;
  tickUpperIndex: number;
  tickSpacing: number;
}): {
  tickArrayLower: PublicKey;
  tickArrayUpper: PublicKey;
} {
  const startLower = getStartTickIndex(
    params.tickLowerIndex,
    params.tickSpacing
  );
  const startUpper = getStartTickIndex(
    params.tickUpperIndex,
    params.tickSpacing
  );

  const [tickArrayLower] = getTickArrayAddress(params.whirlpool, startLower);
  const [tickArrayUpper] = getTickArrayAddress(params.whirlpool, startUpper);

  return { tickArrayLower, tickArrayUpper };
}

// =============================================================================
// EXAMPLE USAGE
// =============================================================================

/**
 * Example: Building the collect_fees instruction with tick arrays.
 *
 * ```typescript
 * import { deriveTickArraysForCollectFees } from "./collectFees";
 *
 * // Read position custody on-chain
 * const custody = await program.account.positionCustody.fetch(custodyPda);
 *
 * // Read whirlpool on-chain for tick_spacing
 * // (or cache it if known — e.g., tick_spacing = 64 for SOL/USDC)
 * const whirlpoolState = await whirlpoolClient.getPool(custody.whirlpool);
 * const tickSpacing = whirlpoolState.tickSpacing;
 *
 * // Derive tick arrays
 * const { tickArrayLower, tickArrayUpper } = deriveTickArraysForCollectFees({
 *   whirlpool: custody.whirlpool,
 *   tickLowerIndex: custody.tickLowerIndex,
 *   tickUpperIndex: custody.tickUpperIndex,
 *   tickSpacing,
 * });
 *
 * // Build instruction
 * const tx = await program.methods
 *   .collectFees()
 *   .accounts({
 *     user: user.publicKey,
 *     config: configPda,
 *     userBondAccount,
 *     bondMint,
 *     positionMint,
 *     positionCustody: custodyPda,
 *     custodyPositionTokenAccount,
 *     whirlpoolPosition,
 *     whirlpool: custody.whirlpool,
 *     userTokenAAccount,
 *     userTokenBAccount,
 *     tokenVaultA,
 *     tokenVaultB,
 *     whirlpoolProgram: WHIRLPOOL_PROGRAM_ID,
 *     tickArrayLower,   // ← NEW (Batch 1)
 *     tickArrayUpper,   // ← NEW (Batch 1)
 *     tokenProgram: TOKEN_PROGRAM_ID,
 *   })
 *   .signers([user])
 *   .rpc();
 * ```
 */
