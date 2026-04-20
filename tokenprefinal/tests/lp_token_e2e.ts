/**
 * LP Token — End-to-End Integration Tests
 *
 * Validates full production lifecycle flows of the LP Token program:
 *   1. Full Mint Flow (initialize → register minter → create ATA → mint)
 *   2. Transfer Flow (user-to-user, works during pause)
 *   3. Delegate + TransferFrom Flow (approve → delegated transfer → revoke)
 *   4. Burn Flow (dual-signer requirement, supply tracking)
 *   5. Pause Behavior (mint/burn blocked, transfer allowed)
 *   6. Ownership Transfer (two-step propose/accept)
 *   7. Metadata Flow (set_metadata via Metaplex CPI)
 *   8. Adversarial integration edge cases
 *
 * All tests use real SPL Token accounts and derive PDAs explicitly.
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { LpToken } from "../target/types/lp_token";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccount,
  getAssociatedTokenAddress,
  getAccount,
  createRevokeInstruction,
} from "@solana/spl-token";
import { assert } from "chai";

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const EVM_CHAIN_ID = new anchor.BN(1);
const DECIMALS = 9;
const ONE_TOKEN = new anchor.BN(1_000_000_000); // 1 token at 9 decimals
const HALF_TOKEN = new anchor.BN(500_000_000);

// Metaplex Token Metadata program ID
const TOKEN_METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

function deriveTokenState(
  mint: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("token_state"), mint.toBuffer()],
    programId
  );
}

function deriveMinterRecord(
  tokenState: PublicKey,
  minter: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("minter"), tokenState.toBuffer(), minter.toBuffer()],
    programId
  );
}

function deriveMetadataPDA(mint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      mint.toBuffer(),
    ],
    TOKEN_METADATA_PROGRAM_ID
  );
}

async function getOrCreateATA(
  connection: anchor.web3.Connection,
  payer: Keypair,
  mint: PublicKey,
  owner: PublicKey
): Promise<PublicKey> {
  const ata = await getAssociatedTokenAddress(mint, owner);
  try {
    await getAccount(connection, ata);
  } catch {
    await createAssociatedTokenAccount(connection, payer, mint, owner);
  }
  return ata;
}

async function airdrop(
  connection: anchor.web3.Connection,
  target: PublicKey,
  lamports = 2_000_000_000
): Promise<void> {
  const sig = await connection.requestAirdrop(target, lamports);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({ signature: sig, ...latest });
}

async function getTokenBalance(
  connection: anchor.web3.Connection,
  tokenAccount: PublicKey
): Promise<bigint> {
  const acct = await getAccount(connection, tokenAccount);
  return acct.amount;
}

async function getTotalSupply(
  connection: anchor.web3.Connection,
  mint: PublicKey
): Promise<bigint> {
  const { value } = await connection.getTokenSupply(mint);
  return BigInt(value.amount);
}

// ─────────────────────────────────────────────────────────────────────────────
// Test suite
// ─────────────────────────────────────────────────────────────────────────────

describe("LP Token — End-to-End Integration Tests", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LpToken as Program<LpToken>;
  const connection = provider.connection;

  // Participants
  let owner: Keypair;
  let minter: Keypair;
  let userA: Keypair;
  let userB: Keypair;
  let attacker: Keypair;

  // Mint and PDA state
  let mintKeypair: Keypair;
  let tokenState: PublicKey;
  let tokenStateBump: number;

  // Token accounts
  let ownerATA: PublicKey;
  let userA_ATA: PublicKey;
  let userB_ATA: PublicKey;
  let attackerATA: PublicKey;

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 1: Full Mint Flow
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 1: Full Mint Lifecycle", () => {
    before(async () => {
      // Generate all participants
      owner = Keypair.generate();
      minter = Keypair.generate();
      userA = Keypair.generate();
      userB = Keypair.generate();
      attacker = Keypair.generate();
      mintKeypair = Keypair.generate();

      // Fund participants
      await Promise.all([
        airdrop(connection, owner.publicKey),
        airdrop(connection, minter.publicKey),
        airdrop(connection, userA.publicKey),
        airdrop(connection, userB.publicKey),
        airdrop(connection, attacker.publicKey),
      ]);

      // Derive PDAs
      [tokenState, tokenStateBump] = deriveTokenState(
        mintKeypair.publicKey,
        program.programId
      );
    });

    it("Step 1: Initialize mint with correct owner and parameters", async () => {
      await program.methods
        .initializeMint({
          owner: owner.publicKey,
          evmChainId: EVM_CHAIN_ID,
          decimals: DECIMALS,
        })
        .accounts({
          payer: owner.publicKey,
          tokenMint: mintKeypair.publicKey,
          tokenState,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([owner, mintKeypair])
        .rpc();

      // Verify state
      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.owner.toBase58(), owner.publicKey.toBase58());
      assert.equal(state.isPaused, false);
      assert.equal(state.evmChainId.toString(), "1");

      // Verify total supply is 0
      const supply = await getTotalSupply(connection, mintKeypair.publicKey);
      assert.equal(supply, 0n);
    });

    it("Step 2: Register a minter", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: minter.publicKey,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      const record = await program.account.minterRecord.fetch(minterRecord);
      assert.equal(record.isActive, true);
      assert.equal(record.minter.toBase58(), minter.publicKey.toBase58());
    });

    it("Step 3: Create ATAs for all participants", async () => {
      ownerATA = await getOrCreateATA(
        connection,
        owner,
        mintKeypair.publicKey,
        owner.publicKey
      );
      userA_ATA = await getOrCreateATA(
        connection,
        owner,
        mintKeypair.publicKey,
        userA.publicKey
      );
      userB_ATA = await getOrCreateATA(
        connection,
        owner,
        mintKeypair.publicKey,
        userB.publicKey
      );
      attackerATA = await getOrCreateATA(
        connection,
        owner,
        mintKeypair.publicKey,
        attacker.publicKey
      );

      // All created with 0 balance
      const balA = await getTokenBalance(connection, userA_ATA);
      assert.equal(balA, 0n);
    });

    it("Step 4a: Minter mints tokens to userA", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      const supplyBefore = await getTotalSupply(connection, mintKeypair.publicKey);

      await program.methods
        .mintTokens(ONE_TOKEN)
        .accounts({
          authority: minter.publicKey,
          tokenState,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: userA_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([minter])
        .rpc();

      // Verify user balance
      const balA = await getTokenBalance(connection, userA_ATA);
      assert.equal(balA, BigInt(ONE_TOKEN.toString()));

      // Verify total supply increased
      const supplyAfter = await getTotalSupply(connection, mintKeypair.publicKey);
      assert.equal(supplyAfter - supplyBefore, BigInt(ONE_TOKEN.toString()));
    });

    it("Step 4b: Owner can also mint (without minter record validation)", async () => {
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      await program.methods
        .mintTokens(ONE_TOKEN)
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: userB_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const balB = await getTokenBalance(connection, userB_ATA);
      assert.equal(balB, BigInt(ONE_TOKEN.toString()));
    });

    it("Step 4c: Non-minter cannot mint tokens", async () => {
      const [attackerRecord] = deriveMinterRecord(
        tokenState,
        attacker.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: attacker.publicKey,
            tokenState,
            minterRecord: attackerRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: attackerATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Non-minter should not be able to mint");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Step 4d: Multiple mints accumulate correctly", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      // Mint 3 more tokens to userA
      await program.methods
        .mintTokens(new anchor.BN(3_000_000_000))
        .accounts({
          authority: minter.publicKey,
          tokenState,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: userA_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([minter])
        .rpc();

      // userA should now have 4 tokens (1 + 3)
      const balA = await getTokenBalance(connection, userA_ATA);
      assert.equal(balA, 4_000_000_000n);
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 2: Transfer Flow
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 2: Transfer Flow", () => {
    it("UserA transfers tokens to UserB", async () => {
      const balABefore = await getTokenBalance(connection, userA_ATA);
      const balBBefore = await getTokenBalance(connection, userB_ATA);

      await program.methods
        .transferTokens(ONE_TOKEN)
        .accounts({
          fromAuthority: userA.publicKey,
          fromTokenAccount: userA_ATA,
          toTokenAccount: userB_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([userA])
        .rpc();

      const balAAfter = await getTokenBalance(connection, userA_ATA);
      const balBAfter = await getTokenBalance(connection, userB_ATA);

      assert.equal(balABefore - balAAfter, BigInt(ONE_TOKEN.toString()));
      assert.equal(balBAfter - balBBefore, BigInt(ONE_TOKEN.toString()));
    });

    it("Transfer fails on insufficient balance", async () => {
      const balance = await getTokenBalance(connection, userA_ATA);
      const tooMuch = new anchor.BN((balance + 1n).toString());

      try {
        await program.methods
          .transferTokens(tooMuch)
          .accounts({
            fromAuthority: userA.publicKey,
            fromTokenAccount: userA_ATA,
            toTokenAccount: userB_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([userA])
          .rpc();
        assert.fail("Transfer should fail with insufficient balance");
      } catch (err: any) {
        // SPL Token error for insufficient funds
        assert.ok(err.toString().length > 0);
      }
    });

    it("Transfer works even when paused (LPToken-specific behavior)", async () => {
      // Pause the contract
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const balBBefore = await getTokenBalance(connection, userB_ATA);

      // Transfer should still succeed — LPToken does not block transfers during pause
      await program.methods
        .transferTokens(HALF_TOKEN)
        .accounts({
          fromAuthority: userA.publicKey,
          fromTokenAccount: userA_ATA,
          toTokenAccount: userB_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([userA])
        .rpc();

      const balBAfter = await getTokenBalance(connection, userB_ATA);
      assert.equal(balBAfter - balBBefore, BigInt(HALF_TOKEN.toString()));

      // Unpause for subsequent tests
      await program.methods
        .setPause(false)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();
    });

    it("Total supply unchanged after transfers", async () => {
      // Transfers don't change total supply — only mint/burn do
      const supply = await getTotalSupply(connection, mintKeypair.publicKey);
      // We minted 1 (to A) + 1 (to B) + 3 (to A) = 5 tokens total
      assert.equal(supply, 5_000_000_000n);
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 3: Delegate + TransferFrom Flow (Allowance Equivalent)
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 3: Delegate + TransferFrom (Allowance)", () => {
    it("UserA approves UserB as delegate", async () => {
      await program.methods
        .approveDelegate(ONE_TOKEN)
        .accounts({
          tokenAccountOwner: userA.publicKey,
          tokenAccount: userA_ATA,
          delegate: userB.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([userA])
        .rpc();

      const acct = await getAccount(connection, userA_ATA);
      assert.equal(acct.delegate?.toBase58(), userB.publicKey.toBase58());
      assert.equal(acct.delegatedAmount.toString(), ONE_TOKEN.toString());
    });

    it("Delegate (UserB) transfers tokens from UserA using allowance", async () => {
      const balBBefore = await getTokenBalance(connection, userB_ATA);

      await program.methods
        .transferTokens(HALF_TOKEN)
        .accounts({
          fromAuthority: userB.publicKey, // delegate
          fromTokenAccount: userA_ATA, // source (delegated)
          toTokenAccount: userB_ATA, // destination
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([userB])
        .rpc();

      const balBAfter = await getTokenBalance(connection, userB_ATA);
      assert.equal(balBAfter - balBBefore, BigInt(HALF_TOKEN.toString()));

      // Delegated amount should have decreased
      const acct = await getAccount(connection, userA_ATA);
      assert.equal(
        acct.delegatedAmount.toString(),
        HALF_TOKEN.toString() // 1 token - 0.5 token = 0.5 token remaining
      );
    });

    it("Delegate transfer fails if exceeding remaining allowance", async () => {
      // Remaining allowance is 0.5 tokens, try to transfer 0.6
      try {
        await program.methods
          .transferTokens(new anchor.BN(600_000_000))
          .accounts({
            fromAuthority: userB.publicKey,
            fromTokenAccount: userA_ATA,
            toTokenAccount: userB_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([userB])
          .rpc();
        assert.fail("Should fail — exceeds remaining allowance");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });

    it("Revoke delegate removes all allowance", async () => {
      // Revoke using SPL Token revoke instruction
      const revokeIx = createRevokeInstruction(
        userA_ATA,
        userA.publicKey,
        [],
        TOKEN_PROGRAM_ID
      );
      const tx = new anchor.web3.Transaction().add(revokeIx);
      await provider.sendAndConfirm(tx, [userA]);

      // Verify delegate is cleared
      const acct = await getAccount(connection, userA_ATA);
      assert.isNull(acct.delegate);
      assert.equal(acct.delegatedAmount.toString(), "0");

      // Delegate can no longer transfer
      try {
        await program.methods
          .transferTokens(new anchor.BN(1))
          .accounts({
            fromAuthority: userB.publicKey,
            fromTokenAccount: userA_ATA,
            toTokenAccount: userB_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([userB])
          .rpc();
        assert.fail("Revoked delegate should not be able to transfer");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 4: Burn Flow (CRITICAL — dual-signer requirement)
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 4: Burn Flow (Dual-Signer)", () => {
    it("Burn fails WITHOUT token_account_authority co-signing", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      // Minter tries to burn from userA without userA signing
      try {
        await program.methods
          .burnTokens(HALF_TOKEN)
          .accounts({
            authority: minter.publicKey,
            tokenAccountAuthority: userA.publicKey,
            tokenState,
            minterRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minter]) // userA NOT signing!
          .rpc();
        assert.fail("Burn should fail without token holder co-signing");
      } catch (err: any) {
        // Transaction should fail because userA didn't sign
        assert.ok(
          err.toString().includes("Signature verification failed") ||
            err.toString().includes("unknown signer") ||
            err.toString().includes("missing signature")
        );
      }
    });

    it("Burn succeeds WITH correct dual signers (minter + token holder)", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      const balBefore = await getTokenBalance(connection, userA_ATA);
      const supplyBefore = await getTotalSupply(connection, mintKeypair.publicKey);

      await program.methods
        .burnTokens(HALF_TOKEN)
        .accounts({
          authority: minter.publicKey,
          tokenAccountAuthority: userA.publicKey,
          tokenState,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          tokenAccount: userA_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([minter, userA]) // both sign
        .rpc();

      const balAfter = await getTokenBalance(connection, userA_ATA);
      const supplyAfter = await getTotalSupply(connection, mintKeypair.publicKey);

      // Balance decreased
      assert.equal(balBefore - balAfter, BigInt(HALF_TOKEN.toString()));
      // Total supply decreased
      assert.equal(supplyBefore - supplyAfter, BigInt(HALF_TOKEN.toString()));
    });

    it("Owner can burn (owner is both authority and token holder)", async () => {
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      // Mint some tokens to owner first
      await program.methods
        .mintTokens(ONE_TOKEN)
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: ownerATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const balBefore = await getTokenBalance(connection, ownerATA);

      // Owner burns own tokens — both authority and tokenAccountAuthority are owner
      await program.methods
        .burnTokens(HALF_TOKEN)
        .accounts({
          authority: owner.publicKey,
          tokenAccountAuthority: owner.publicKey,
          tokenState,
          minterRecord: dummyRecord,
          tokenMint: mintKeypair.publicKey,
          tokenAccount: ownerATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const balAfter = await getTokenBalance(connection, ownerATA);
      assert.equal(balBefore - balAfter, BigInt(HALF_TOKEN.toString()));
    });

    it("Unauthorized user cannot burn even with token holder co-signing", async () => {
      const [attackerRecord] = deriveMinterRecord(
        tokenState,
        attacker.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: attacker.publicKey, // not owner or minter
            tokenAccountAuthority: userA.publicKey,
            tokenState,
            minterRecord: attackerRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([attacker, userA])
          .rpc();
        assert.fail("Unauthorized user should not be able to burn");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 5: Pause Behavior
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 5: Pause Behavior (EVM LPToken Exact)", () => {
    it("Owner pauses — mint fails", async () => {
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: minter.publicKey,
            tokenState,
            minterRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minter])
          .rpc();
        assert.fail("Mint should fail when paused");
      } catch (err: any) {
        assert.include(err.toString(), "Paused");
      }
    });

    it("Owner pauses — burn fails", async () => {
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: owner.publicKey,
            tokenAccountAuthority: owner.publicKey,
            tokenState,
            minterRecord: dummyRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: ownerATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Burn should fail when paused");
      } catch (err: any) {
        assert.include(err.toString(), "Paused");
      }
    });

    it("Transfer STILL works during pause (LPToken behavior)", async () => {
      // Verify we're still paused
      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, true);

      const balBefore = await getTokenBalance(connection, userB_ATA);

      await program.methods
        .transferTokens(new anchor.BN(100))
        .accounts({
          fromAuthority: userA.publicKey,
          fromTokenAccount: userA_ATA,
          toTokenAccount: userB_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([userA])
        .rpc();

      const balAfter = await getTokenBalance(connection, userB_ATA);
      assert.equal(balAfter - balBefore, 100n);
    });

    it("Approve STILL works during pause (LPToken behavior)", async () => {
      await program.methods
        .approveDelegate(ONE_TOKEN)
        .accounts({
          tokenAccountOwner: userA.publicKey,
          tokenAccount: userA_ATA,
          delegate: userB.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([userA])
        .rpc();

      const acct = await getAccount(connection, userA_ATA);
      assert.equal(acct.delegate?.toBase58(), userB.publicKey.toBase58());
    });

    it("Unpause restores normal operation", async () => {
      await program.methods
        .setPause(false)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Mint should work again
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      await program.methods
        .mintTokens(ONE_TOKEN)
        .accounts({
          authority: minter.publicKey,
          tokenState,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: userA_ATA,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([minter])
        .rpc();

      // No error = success
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 6: Ownership Transfer (Two-Step)
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 6: Ownership Transfer (Two-Step)", () => {
    let newOwner: Keypair;

    before(async () => {
      newOwner = Keypair.generate();
      await airdrop(connection, newOwner.publicKey);
    });

    it("Step 1: Owner proposes new owner", async () => {
      await program.methods
        .transferOwnership(newOwner.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.pendingOwner.toBase58(), newOwner.publicKey.toBase58());
      // Owner hasn't changed yet
      assert.equal(state.owner.toBase58(), owner.publicKey.toBase58());
    });

    it("Stranger cannot accept ownership meant for newOwner", async () => {
      try {
        await program.methods
          .acceptOwnership()
          .accounts({
            newOwner: attacker.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Stranger should not accept ownership");
      } catch (err: any) {
        assert.include(err.toString(), "NoPendingOwnership");
      }
    });

    it("Cannot accept without proposal (from scratch)", async () => {
      // userA has never been proposed
      try {
        await program.methods
          .acceptOwnership()
          .accounts({
            newOwner: userA.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([userA])
          .rpc();
        assert.fail("Should fail — no proposal for userA");
      } catch (err: any) {
        assert.include(err.toString(), "NoPendingOwnership");
      }
    });

    it("Step 2: New owner accepts ownership", async () => {
      await program.methods
        .acceptOwnership()
        .accounts({
          newOwner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.owner.toBase58(), newOwner.publicKey.toBase58());
      assert.equal(state.pendingOwner.toBase58(), PublicKey.default.toBase58());
    });

    it("Old owner loses all privileges", async () => {
      // Old owner tries to pause
      try {
        await program.methods
          .setPause(true)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
        assert.fail("Old owner should not have privileges");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }

      // Old owner tries to update minter
      const someMinter = Keypair.generate();
      const [someRecord] = deriveMinterRecord(
        tokenState,
        someMinter.publicKey,
        program.programId
      );
      try {
        await program.methods
          .updateMinter({ isActive: true })
          .accounts({
            owner: owner.publicKey,
            tokenState,
            targetMinter: someMinter.publicKey,
            minterRecord: someRecord,
            tokenMint: mintKeypair.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([owner])
          .rpc();
        assert.fail("Old owner should not update minter");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("New owner can pause/unpause", async () => {
      await program.methods
        .setPause(true)
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      let state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, true);

      await program.methods
        .setPause(false)
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, false);
    });

    it("New owner can update minters", async () => {
      const anotherMinter = Keypair.generate();
      const [anotherRecord] = deriveMinterRecord(
        tokenState,
        anotherMinter.publicKey,
        program.programId
      );

      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          targetMinter: anotherMinter.publicKey,
          minterRecord: anotherRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([newOwner])
        .rpc();

      const record = await program.account.minterRecord.fetch(anotherRecord);
      assert.equal(record.isActive, true);
    });

    it("Cleanup: transfer ownership back to original owner", async () => {
      await program.methods
        .transferOwnership(owner.publicKey)
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      await program.methods
        .acceptOwnership()
        .accounts({
          newOwner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.owner.toBase58(), owner.publicKey.toBase58());
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 7: Metadata Flow (NEW FEATURE)
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 7: Token Metadata (set_metadata)", () => {
    it("Only owner can call set_metadata", async () => {
      const [metadataPDA] = deriveMetadataPDA(mintKeypair.publicKey);

      try {
        await program.methods
          .setMetadata({
            name: "Evil Token",
            symbol: "EVIL",
            uri: "https://evil.com/metadata.json",
          })
          .accounts({
            owner: attacker.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
            metadata: metadataPDA,
            tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Non-owner should not be able to set metadata");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Owner can create metadata — CPI reaches Metaplex (requires devnet/mainnet)", async () => {
      const [metadataPDA] = deriveMetadataPDA(mintKeypair.publicKey);

      // On localnet, the Metaplex Token Metadata program is not deployed.
      // This test verifies:
      //   1. Owner auth passes (non-owner test above proves access control)
      //   2. PDA derivation is correct
      //   3. The CPI is correctly constructed (reaches "Unsupported program id"
      //      because Metaplex isn't on localnet, NOT a constraint/signer error)
      //
      // On devnet with Metaplex deployed, this test would pass fully.
      try {
        await program.methods
          .setMetadata({
            name: "LP Token",
            symbol: "LP",
            uri: "https://energi.world/lp-token-metadata.json",
          })
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
            metadata: metadataPDA,
            tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .signers([owner])
          .rpc();

        // If we get here, Metaplex IS available (e.g. devnet) — verify metadata
        const metadataInfo = await connection.getAccountInfo(metadataPDA);
        assert.isNotNull(metadataInfo, "Metadata account should exist");
        assert.equal(
          metadataInfo!.owner.toBase58(),
          TOKEN_METADATA_PROGRAM_ID.toBase58()
        );
      } catch (err: any) {
        // Expected on localnet: Metaplex program not deployed
        // Verify it's the CPI failure (not an auth or constraint error)
        const errStr = err.toString();
        assert.include(errStr, "Unsupported program id");
        // This confirms the instruction passed all on-chain validation
        // and the CPI was correctly issued to the Metaplex program address
      }
    });

    it("Owner can update metadata — CPI reaches Metaplex (requires devnet/mainnet)", async () => {
      const [metadataPDA] = deriveMetadataPDA(mintKeypair.publicKey);

      try {
        await program.methods
          .setMetadata({
            name: "LP Token V2",
            symbol: "LPv2",
            uri: "https://energi.world/lp-token-metadata-v2.json",
          })
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
            metadata: metadataPDA,
            tokenMetadataProgram: TOKEN_METADATA_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .signers([owner])
          .rpc();

        // If on devnet, verify metadata was updated
        const metadataInfo = await connection.getAccountInfo(metadataPDA);
        assert.isNotNull(metadataInfo);
      } catch (err: any) {
        // Expected on localnet — Metaplex not deployed
        assert.include(err.toString(), "Unsupported program id");
      }
    });

    it("Metadata PDA is correctly derived for the mint", async () => {
      const [metadataPDA] = deriveMetadataPDA(mintKeypair.publicKey);

      // Verify PDA derivation matches expected seeds
      const [expectedPDA] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("metadata"),
          TOKEN_METADATA_PROGRAM_ID.toBuffer(),
          mintKeypair.publicKey.toBuffer(),
        ],
        TOKEN_METADATA_PROGRAM_ID
      );
      assert.equal(metadataPDA.toBase58(), expectedPDA.toBase58());
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // FLOW 8: Adversarial Integration Edge Cases
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Flow 8: Adversarial Integration Edge Cases", () => {
    // Create a second independent mint for cross-mint attack vectors
    let mintB: Keypair;
    let tokenStateB: PublicKey;
    let userA_ATA_B: PublicKey;

    before(async () => {
      mintB = Keypair.generate();
      [tokenStateB] = deriveTokenState(mintB.publicKey, program.programId);

      await program.methods
        .initializeMint({
          owner: owner.publicKey,
          evmChainId: new anchor.BN(2),
          decimals: DECIMALS,
        })
        .accounts({
          payer: owner.publicKey,
          tokenMint: mintB.publicKey,
          tokenState: tokenStateB,
          tokenProgram: TOKEN_PROGRAM_ID,
          systemProgram: SystemProgram.programId,
          rent: SYSVAR_RENT_PUBKEY,
        })
        .signers([owner, mintB])
        .rpc();

      userA_ATA_B = await getOrCreateATA(
        connection,
        owner,
        mintB.publicKey,
        userA.publicKey
      );
    });

    it("Cross-mint: Cannot mint to wrong mint's token account", async () => {
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: owner.publicKey,
            tokenState, // mint A's state
            minterRecord: dummyRecord,
            tokenMint: mintKeypair.publicKey, // mint A
            recipientTokenAccount: userA_ATA_B, // belongs to mint B!
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Should fail — token account mint mismatch");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidMint");
      }
    });

    it("Cross-mint: Cannot use tokenState from mint B with mint A", async () => {
      const [dummyRecordB] = deriveMinterRecord(
        tokenStateB,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: owner.publicKey,
            tokenState: tokenStateB, // mint B's state
            minterRecord: dummyRecordB,
            tokenMint: mintKeypair.publicKey, // mint A — PDA seeds won't match
            recipientTokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Should fail — PDA seeds mismatch");
      } catch (err: any) {
        // Anchor constraint: seeds don't match
        assert.ok(err.toString().length > 0);
      }
    });

    it("Cross-mint: Minter record from mint A invalid on mint B", async () => {
      // Minter is registered on mint A, tries to use that record on mint B
      const [minterRecordA] = deriveMinterRecord(
        tokenState, // mint A
        minter.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: minter.publicKey,
            tokenState: tokenStateB, // mint B
            minterRecord: minterRecordA, // from mint A!
            tokenMint: mintB.publicKey,
            recipientTokenAccount: userA_ATA_B,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minter])
          .rpc();
        assert.fail("Should fail — cross-mint minter record");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Wrong signer: Attacker cannot sign as someone else's authority", async () => {
      // Attacker passes minter's record but signs as attacker
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: attacker.publicKey, // wrong signer
            tokenState,
            minterRecord, // minter's record
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: attackerATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Should fail — authority doesn't match minter record");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Burn with wrong token_account_authority", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minter.publicKey,
        program.programId
      );

      // Minter tries to burn from userA's account but claims attacker is the authority
      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: minter.publicKey,
            tokenAccountAuthority: attacker.publicKey, // wrong! userA owns the ATA
            tokenState,
            minterRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minter, attacker])
          .rpc();
        assert.fail("Should fail — wrong token account authority");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidTokenAuthority");
      }
    });

    it("Delegate misuse: Non-delegate stranger cannot transfer", async () => {
      try {
        await program.methods
          .transferTokens(new anchor.BN(1))
          .accounts({
            fromAuthority: attacker.publicKey,
            fromTokenAccount: userA_ATA,
            toTokenAccount: attackerATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([attacker])
          .rpc();
        assert.fail("Non-delegate should not transfer");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });

    it("Re-initialization of existing mint is impossible", async () => {
      try {
        await program.methods
          .initializeMint({
            owner: attacker.publicKey,
            evmChainId: new anchor.BN(999),
            decimals: DECIMALS,
          })
          .accounts({
            payer: attacker.publicKey,
            tokenMint: mintKeypair.publicKey, // already exists
            tokenState,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .signers([attacker, mintKeypair])
          .rpc();
        assert.fail("Re-initialization should fail");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });

    it("Deregistered minter cannot mint or burn", async () => {
      // Create and deregister a minter
      const tempMinter = Keypair.generate();
      await airdrop(connection, tempMinter.publicKey);
      const [tempRecord] = deriveMinterRecord(
        tokenState,
        tempMinter.publicKey,
        program.programId
      );

      // Register
      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: tempMinter.publicKey,
          minterRecord: tempRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Deregister
      await program.methods
        .updateMinter({ isActive: false })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: tempMinter.publicKey,
          minterRecord: tempRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Try to mint
      try {
        await program.methods
          .mintTokens(ONE_TOKEN)
          .accounts({
            authority: tempMinter.publicKey,
            tokenState,
            minterRecord: tempRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([tempMinter])
          .rpc();
        assert.fail("Deregistered minter should not mint");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }

      // Try to burn
      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: tempMinter.publicKey,
            tokenAccountAuthority: userA.publicKey,
            tokenState,
            minterRecord: tempRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: userA_ATA,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([tempMinter, userA])
          .rpc();
        assert.fail("Deregistered minter should not burn");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Supply accounting is consistent after multiple operations", async () => {
      // Get current supply and verify it matches expected state
      const supply = await getTotalSupply(connection, mintKeypair.publicKey);

      // Get all balances
      const balOwner = await getTokenBalance(connection, ownerATA);
      const balA = await getTokenBalance(connection, userA_ATA);
      const balB = await getTokenBalance(connection, userB_ATA);
      const balAttacker = await getTokenBalance(connection, attackerATA);

      // Sum of all balances should equal total supply
      const totalBalances = balOwner + balA + balB + balAttacker;
      assert.equal(totalBalances, supply);
    });
  });
});
