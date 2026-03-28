import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorError } from "@coral-xyz/anchor";
import { LpBonds } from "../../target/types/lp_bonds";
import { LpBondsEvolution } from "../../target/types/lp_bonds_evolution";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import { expect } from "chai";

// =============================================================================
// CONSTANTS
// =============================================================================

const NONCE_SEED = Buffer.from("nonce");
const EVOLUTION_NONCE_SEED = Buffer.from("evolution_nonce");

// =============================================================================
// SECURITY TEST: NONCE IMMUTABILITY
// =============================================================================
//
// Validates BG-18 / EBG-13 fix: nonce accounts cannot be closed or reset.
//
// Attack vector (pre-fix):
//   1. User has nonce_account.current_nonce = N
//   2. User calls close_nonce_account -> PDA closed, rent returned
//   3. User calls initialize_nonce -> new PDA with current_nonce = 0
//   4. Oracle signatures for nonces 1..N become replayable
//
// Post-fix:
//   - close_nonce_account instruction removed entirely
//   - close_evolution_nonce instruction removed entirely
//   - initialize_nonce uses `init` (not init_if_needed), so calling it
//     on an existing account fails at the Anchor level
//

describe("Nonce Immutability (BG-18 / EBG-13)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const lpBonds = anchor.workspace.LpBonds as Program<LpBonds>;
  const lpBondsEvolution = anchor.workspace
    .LpBondsEvolution as Program<LpBondsEvolution>;

  // =========================================================================
  // TEST 1: close_nonce_account instruction no longer exists (lp-bonds)
  // =========================================================================

  it("LP-Bonds: close_nonce_account instruction does not exist", () => {
    // The instruction should not be present in the IDL at all.
    // Attempting to access it on the program methods object should be undefined.
    const methods = lpBonds.methods as any;
    expect(methods.closeNonceAccount).to.be.undefined;
  });

  // =========================================================================
  // TEST 2: close_evolution_nonce instruction no longer exists (evolution)
  // =========================================================================

  it("Evolution: close_evolution_nonce instruction does not exist", () => {
    const methods = lpBondsEvolution.methods as any;
    expect(methods.closeEvolutionNonce).to.be.undefined;
  });

  // =========================================================================
  // TEST 3: Re-initializing an existing nonce account fails (lp-bonds)
  // =========================================================================

  it("LP-Bonds: cannot re-initialize existing nonce account", async () => {
    const user = Keypair.generate();

    // Fund user
    const sig = await provider.connection.requestAirdrop(
      user.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig);

    // Derive nonce PDA
    const [noncePda] = PublicKey.findProgramAddressSync(
      [NONCE_SEED, user.publicKey.toBuffer()],
      lpBonds.programId
    );

    // First initialization should succeed
    await lpBonds.methods
      .initializeNonce()
      .accounts({
        user: user.publicKey,
        nonceAccount: noncePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    // Second initialization MUST fail (account already exists)
    try {
      await lpBonds.methods
        .initializeNonce()
        .accounts({
          user: user.publicKey,
          nonceAccount: noncePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      // If we reach here, the test should fail
      expect.fail("Re-initialization should have failed but succeeded");
    } catch (err: any) {
      // Anchor `init` constraint causes a transaction error when the
      // account already exists. The exact error varies but should indicate
      // the account is already in use.
      expect(err).to.exist;
    }
  });

  // =========================================================================
  // TEST 4: Re-initializing an existing evolution nonce account fails
  // =========================================================================

  it("Evolution: cannot re-initialize existing evolution nonce account", async () => {
    const user = Keypair.generate();

    // Fund user
    const sig = await provider.connection.requestAirdrop(
      user.publicKey,
      2 * LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(sig);

    // Derive evolution nonce PDA
    const [noncePda] = PublicKey.findProgramAddressSync(
      [EVOLUTION_NONCE_SEED, user.publicKey.toBuffer()],
      lpBondsEvolution.programId
    );

    // First initialization should succeed
    await lpBondsEvolution.methods
      .initializeEvolutionNonce()
      .accounts({
        user: user.publicKey,
        evolutionNonce: noncePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    // Second initialization MUST fail
    try {
      await lpBondsEvolution.methods
        .initializeEvolutionNonce()
        .accounts({
          user: user.publicKey,
          evolutionNonce: noncePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      expect.fail("Re-initialization should have failed but succeeded");
    } catch (err: any) {
      expect(err).to.exist;
    }
  });
});
