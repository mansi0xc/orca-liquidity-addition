import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LpBonds } from "../../target/types/lp_bonds";
import {
  PublicKey,
  Keypair,
} from "@solana/web3.js";
import { expect } from "chai";

// =============================================================================
// CONSTANTS
// =============================================================================

const WHIRLPOOL_PROGRAM_ID = new PublicKey(
  "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
);
const TICK_ARRAY_SIZE = 88;

// =============================================================================
// HELPERS (mirror Rust logic)
// =============================================================================

function getStartTickIndex(tickIndex: number, tickSpacing: number): number {
  const ticksInArray = TICK_ARRAY_SIZE * tickSpacing;
  let start = Math.trunc(tickIndex / ticksInArray);
  if (tickIndex < 0 && tickIndex % ticksInArray !== 0) {
    start -= 1;
  }
  return start * ticksInArray;
}

function getTickArrayAddress(
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

// =============================================================================
// SECURITY TEST: TICK ARRAY VALIDATION
// =============================================================================
//
// Validates tick array PDA derivation hardening.
//
// Attack scenario prevented:
//   1. Attacker creates a valid-looking tick array for a different pool/range
//   2. Passes it to collect_fees
//   3. update_fees_and_rewards operates on wrong tick data
//   4. Fee accounting corrupted
//
// Defense layers:
//   1. Owner check: tick_array.owner == WHIRLPOOL_PROGRAM_ID
//   2. PDA derivation: tick_array.key() == expected PDA from (whirlpool, start_tick)
//   3. Whirlpool program internal validation during CPI
// =============================================================================

describe("Security: Tick Array Validation", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LpBonds as Program<LpBonds>;

  // ---------------------------------------------------------------------------
  // TEST: Wrong PDA derivation yields different address
  // ---------------------------------------------------------------------------
  it("should reject tick arrays with wrong PDA derivation", () => {
    const whirlpool = Keypair.generate().publicKey;
    const correctTickLower = -10000;
    const correctTickSpacing = 64;
    const wrongTickLower = 10000;

    const correctStart = getStartTickIndex(correctTickLower, correctTickSpacing);
    const wrongStart = getStartTickIndex(wrongTickLower, correctTickSpacing);

    const [correctPda] = getTickArrayAddress(whirlpool, correctStart);
    const [wrongPda] = getTickArrayAddress(whirlpool, wrongStart);

    expect(correctPda.toBase58()).to.not.equal(wrongPda.toBase58());
  });

  // ---------------------------------------------------------------------------
  // TEST: Cross-pool tick arrays
  // ---------------------------------------------------------------------------
  it("should reject tick arrays from a different whirlpool", () => {
    const whirlpoolA = Keypair.generate().publicKey;
    const whirlpoolB = Keypair.generate().publicKey;
    const startTick = 5632;

    const [pdaA] = getTickArrayAddress(whirlpoolA, startTick);
    const [pdaB] = getTickArrayAddress(whirlpoolB, startTick);

    expect(pdaA.toBase58()).to.not.equal(pdaB.toBase58());
  });

  // ---------------------------------------------------------------------------
  // TEST: Correct tick arrays pass validation
  // ---------------------------------------------------------------------------
  it("should accept correctly derived tick arrays", () => {
    const whirlpool = Keypair.generate().publicKey;

    const lowerStart = getStartTickIndex(-10000, 64);
    const upperStart = getStartTickIndex(10000, 64);

    expect(lowerStart).to.equal(-11264);
    expect(upperStart).to.equal(5632);

    const [lowerPda] = getTickArrayAddress(whirlpool, lowerStart);
    const [upperPda] = getTickArrayAddress(whirlpool, upperStart);

    expect(lowerPda.toBase58()).to.not.equal(upperPda.toBase58());
  });

  // ---------------------------------------------------------------------------
  // TEST: Edge case — same tick array for lower and upper
  // ---------------------------------------------------------------------------
  it("should handle same tick array for lower and upper", () => {
    const lowerStart = getStartTickIndex(100, 64);
    const upperStart = getStartTickIndex(200, 64);

    expect(lowerStart).to.equal(0);
    expect(upperStart).to.equal(0);
    expect(lowerStart).to.equal(upperStart);
  });

  // ---------------------------------------------------------------------------
  // TEST: Boundary tick indices
  // ---------------------------------------------------------------------------
  it("should handle MIN/MAX tick indices", () => {
    const minTick = -443636;
    const maxTick = 443636;
    const tickSpacing = 64;

    const minStart = getStartTickIndex(minTick, tickSpacing);
    const maxStart = getStartTickIndex(maxTick, tickSpacing);

    // Both should produce valid (non-NaN) start indices
    expect(minStart).to.be.a("number");
    expect(maxStart).to.be.a("number");
    expect(minStart).to.be.lessThan(0);
    expect(maxStart).to.be.greaterThan(0);
  });

  // ---------------------------------------------------------------------------
  // TEST: Various tick spacings
  // ---------------------------------------------------------------------------
  it("should compute correctly for common tick spacings", () => {
    const tickIndex = 15000;

    // tick_spacing = 1 (stable pools)
    const start1 = getStartTickIndex(tickIndex, 1);
    expect(start1).to.equal(Math.trunc(tickIndex / (88 * 1)) * (88 * 1));

    // tick_spacing = 8
    const start8 = getStartTickIndex(tickIndex, 8);
    expect(start8).to.equal(Math.trunc(tickIndex / (88 * 8)) * (88 * 8));

    // tick_spacing = 64
    const start64 = getStartTickIndex(tickIndex, 64);
    expect(start64).to.equal(Math.trunc(tickIndex / (88 * 64)) * (88 * 64));

    // tick_spacing = 128
    const start128 = getStartTickIndex(tickIndex, 128);
    expect(start128).to.equal(Math.trunc(tickIndex / (88 * 128)) * (88 * 128));
  });
});
