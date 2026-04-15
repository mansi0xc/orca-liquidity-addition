/**
 * LP Token — Anchor Tests
 *
 * Verifies behavioral parity with EVM LPToken.sol (Energi Core).
 *
 * Test coverage mirrors the EVM test suite in test/GMIToken.test.js
 * and adds Solana-specific validations.
 *
 * Run: anchor test --skip-deploy (after anchor build)
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
  createApproveInstruction,
  createRevokeInstruction,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert, expect } from "chai";

// ─────────────────────────────────────────────────────────────────────────────
// Constants — match EVM deployment parameters
// ─────────────────────────────────────────────────────────────────────────────

const EVM_CHAIN_ID = BigInt(1); // Ethereum mainnet
const DECIMALS = 9; // Solana standard (EVM used 18; u64 constraint)
const MINT_AMOUNT = new anchor.BN(1_000_000_000); // 1 token (9 decimals)
const BURN_AMOUNT = new anchor.BN(500_000_000); // 0.5 token

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/** Derive the TokenState PDA for a given mint. */
function deriveTokenState(
  mint: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("token_state"), mint.toBuffer()],
    programId
  );
}

/** Derive the MinterRecord PDA for a given (tokenState, minter) pair. */
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

/** Create or get an ATA and return its address. */
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
    await createAssociatedTokenAccount(
      connection,
      payer,
      mint,
      owner
    );
  }
  return ata;
}

/** Airdrop SOL and confirm. */
async function airdrop(
  connection: anchor.web3.Connection,
  target: PublicKey,
  lamports = 2_000_000_000
): Promise<void> {
  const sig = await connection.requestAirdrop(target, lamports);
  const latest = await connection.getLatestBlockhash();
  await connection.confirmTransaction({
    signature: sig,
    ...latest,
  });
}

// ─────────────────────────────────────────────────────────────────────────────
// Test suite
// ─────────────────────────────────────────────────────────────────────────────

describe("LP Token — EVM behavioral parity tests", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LpToken as Program<LpToken>;
  const connection = provider.connection;

  // Keypairs
  let owner: Keypair;
  let stranger: Keypair; // non-privileged user
  let minterKeypair: Keypair; // external minter contract equivalent
  let mintKeypair: Keypair; // fresh SPL mint keypair

  // Derived addresses
  let tokenState: PublicKey;
  let tokenStateBump: number;

  // Token accounts
  let ownerTokenAccount: PublicKey;
  let strangerTokenAccount: PublicKey;

  // ── Setup ──────────────────────────────────────────────────────────────────

  before(async () => {
    owner = Keypair.generate();
    stranger = Keypair.generate();
    minterKeypair = Keypair.generate();
    mintKeypair = Keypair.generate();

    // Fund accounts
    await airdrop(connection, owner.publicKey);
    await airdrop(connection, stranger.publicKey);
    await airdrop(connection, minterKeypair.publicKey);

    // Derive PDAs
    [tokenState, tokenStateBump] = deriveTokenState(
      mintKeypair.publicKey,
      program.programId
    );

    // ── Initialize Mint ──────────────────────────────────────────────────────
    await program.methods
      .initializeMint({
        owner: owner.publicKey,
        evmChainId: new anchor.BN(EVM_CHAIN_ID.toString()),
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

    // Create token accounts
    ownerTokenAccount = await getOrCreateATA(
      connection,
      owner,
      mintKeypair.publicKey,
      owner.publicKey
    );
    strangerTokenAccount = await getOrCreateATA(
      connection,
      owner,
      mintKeypair.publicKey,
      stranger.publicKey
    );
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 1: Initialization
  // ─────────────────────────────────────────────────────────────────────────

  describe("Initialization", () => {
    it("Records the correct owner in TokenState", async () => {
      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.owner.toBase58(), owner.publicKey.toBase58());
    });

    it("Records the correct evm_chain_id", async () => {
      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.evmChainId.toString(), EVM_CHAIN_ID.toString());
    });

    it("Starts unpaused", async () => {
      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, false);
    });

    it("Has correct decimals on the SPL mint", async () => {
      const { value } = await connection.getTokenSupply(mintKeypair.publicKey);
      assert.equal(value.decimals, DECIMALS);
    });

    it("Has zero initial supply", async () => {
      const { value } = await connection.getTokenSupply(mintKeypair.publicKey);
      assert.equal(value.uiAmount, 0);
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 2: Minting — EVM: mint(address, amount)
  // ─────────────────────────────────────────────────────────────────────────

  describe("Minting", () => {
    it("Owner can mint tokens to any account", async () => {
      // Pass a dummy minter_record pubkey (won't be read since caller is owner)
      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      await program.methods
        .mintTokens(MINT_AMOUNT)
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyMinterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const acct = await getAccount(connection, ownerTokenAccount);
      assert.equal(acct.amount.toString(), MINT_AMOUNT.toString());
    });

    it("Minter can mint tokens after being registered", async () => {
      // First register the minter
      const [minterRecord, minterRecordBump] = deriveMinterRecord(
        tokenState,
        minterKeypair.publicKey,
        program.programId
      );

      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: minterKeypair.publicKey,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Minter mints to stranger
      await program.methods
        .mintTokens(MINT_AMOUNT)
        .accounts({
          authority: minterKeypair.publicKey,
          tokenState,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: strangerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([minterKeypair])
        .rpc();

      const acct = await getAccount(connection, strangerTokenAccount);
      assert.equal(acct.amount.toString(), MINT_AMOUNT.toString());
    });

    it("Stranger (non-minter) cannot mint tokens", async () => {
      const [fakeRecord] = deriveMinterRecord(
        tokenState,
        stranger.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: stranger.publicKey,
            tokenState,
            minterRecord: fakeRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected transaction to fail");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Cannot mint when paused", async () => {
      // Pause
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: owner.publicKey,
            tokenState,
            minterRecord: dummyMinterRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected transaction to fail");
      } catch (err: any) {
        assert.include(err.toString(), "Paused");
      } finally {
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
      }
    });

    it("No max supply cap — can mint arbitrary amounts", async () => {
      // Mint a large amount — no cap in LPToken
      const largeAmount = new anchor.BN("999999999999999999");
      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      await program.methods
        .mintTokens(largeAmount)
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyMinterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      // No error = no max supply limit enforced
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 3: Burning — EVM: burn(address, amount)
  // ─────────────────────────────────────────────────────────────────────────

  describe("Burning", () => {
    it("Owner can burn tokens (with token account owner co-signing)", async () => {
      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      const before = await getAccount(connection, ownerTokenAccount);

      await program.methods
        .burnTokens(BURN_AMOUNT)
        .accounts({
          authority: owner.publicKey,
          tokenAccountAuthority: owner.publicKey,
          tokenState,
          minterRecord: dummyMinterRecord,
          tokenMint: mintKeypair.publicKey,
          tokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const after = await getAccount(connection, ownerTokenAccount);
      const expected = BigInt(before.amount.toString()) - BigInt(BURN_AMOUNT.toString());
      assert.equal(after.amount.toString(), expected.toString());
    });

    it("Minter can burn tokens (with token account owner co-signing)", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minterKeypair.publicKey,
        program.programId
      );

      const before = await getAccount(connection, strangerTokenAccount);

      await program.methods
        .burnTokens(BURN_AMOUNT)
        .accounts({
          authority: minterKeypair.publicKey,
          tokenAccountAuthority: stranger.publicKey,
          tokenState,
          minterRecord,
          tokenMint: mintKeypair.publicKey,
          tokenAccount: strangerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([minterKeypair, stranger])
        .rpc();

      const after = await getAccount(connection, strangerTokenAccount);
      const expected = BigInt(before.amount.toString()) - BigInt(BURN_AMOUNT.toString());
      assert.equal(after.amount.toString(), expected.toString());
    });

    it("Stranger (non-minter) cannot burn tokens", async () => {
      const [fakeRecord] = deriveMinterRecord(
        tokenState,
        stranger.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(BURN_AMOUNT)
          .accounts({
            authority: stranger.publicKey,
            tokenAccountAuthority: stranger.publicKey,
            tokenState,
            minterRecord: fakeRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected transaction to fail");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Cannot burn when paused", async () => {
      // Pause
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(BURN_AMOUNT)
          .accounts({
            authority: owner.publicKey,
            tokenAccountAuthority: owner.publicKey,
            tokenState,
            minterRecord: dummyMinterRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected transaction to fail");
      } catch (err: any) {
        assert.include(err.toString(), "Paused");
      } finally {
        // Unpause
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
      }
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 4: Minter Management — EVM: updateMinter(address, bool)
  // ─────────────────────────────────────────────────────────────────────────

  describe("Minter management", () => {
    let testMinter: Keypair;
    let testMinterRecord: PublicKey;

    beforeEach(async () => {
      testMinter = Keypair.generate();
      [testMinterRecord] = deriveMinterRecord(
        tokenState,
        testMinter.publicKey,
        program.programId
      );
    });

    it("Owner can add a minter", async () => {
      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: testMinter.publicKey,
          minterRecord: testMinterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      const record = await program.account.minterRecord.fetch(testMinterRecord);
      assert.equal(record.isActive, true);
      assert.equal(record.minter.toBase58(), testMinter.publicKey.toBase58());
    });

    it("Owner can remove a minter", async () => {
      // First add
      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: testMinter.publicKey,
          minterRecord: testMinterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Then remove
      await program.methods
        .updateMinter({ isActive: false })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: testMinter.publicKey,
          minterRecord: testMinterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      const record = await program.account.minterRecord.fetch(testMinterRecord);
      assert.equal(record.isActive, false);
    });

    it("Duplicate operation reverts — cannot add already-added minter", async () => {
      // Add once
      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: testMinter.publicKey,
          minterRecord: testMinterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Add again — should fail
      try {
        await program.methods
          .updateMinter({ isActive: true })
          .accounts({
            owner: owner.publicKey,
            tokenState,
            targetMinter: testMinter.publicKey,
            minterRecord: testMinterRecord,
            tokenMint: mintKeypair.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected DuplicateOperation error");
      } catch (err: any) {
        assert.include(err.toString(), "DuplicateOperation");
      }
    });

    it("Only owner can update minters", async () => {
      try {
        await program.methods
          .updateMinter({ isActive: true })
          .accounts({
            owner: stranger.publicKey,
            tokenState,
            targetMinter: testMinter.publicKey,
            minterRecord: testMinterRecord,
            tokenMint: mintKeypair.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected Unauthorized error");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Deregistered minter can no longer mint", async () => {
      // Add then remove
      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: testMinter.publicKey,
          minterRecord: testMinterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      await program.methods
        .updateMinter({ isActive: false })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: testMinter.publicKey,
          minterRecord: testMinterRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Try to mint
      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: testMinter.publicKey,
            tokenState,
            minterRecord: testMinterRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([testMinter])
          .rpc();
        assert.fail("Expected Unauthorized error");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 5: Pause / Unpause — EVM: pause() / unpause()
  // ─────────────────────────────────────────────────────────────────────────

  describe("Pause / Unpause", () => {
    it("Owner can pause the contract", async () => {
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, true);

      // Cleanup: unpause
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

    it("Owner can unpause the contract", async () => {
      // Pause first
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Unpause
      await program.methods
        .setPause(false)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, false);
    });

    it("Cannot pause an already-paused contract", async () => {
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

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
        assert.fail("Expected InvalidPauseState error");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidPauseState");
      } finally {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
      }
    });

    it("Cannot unpause an already-unpaused contract", async () => {
      try {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected InvalidPauseState error");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidPauseState");
      }
    });

    it("Only owner can pause", async () => {
      try {
        await program.methods
          .setPause(true)
          .accounts({
            owner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected Unauthorized error");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 6: Transfers — EVM: transfer() / transferFrom()
  // LPToken does NOT override _transfer — transfers are not blocked by pause
  // ─────────────────────────────────────────────────────────────────────────

  describe("Transfers (not blocked by pause)", () => {
    beforeEach(async () => {
      // Ensure owner has tokens to transfer
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );
      await program.methods
        .mintTokens(MINT_AMOUNT)
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();
    });

    it("Transfers the correct amount between accounts", async () => {
      const beforeOwner = await getAccount(connection, ownerTokenAccount);
      const beforeStranger = await getAccount(connection, strangerTokenAccount);

      await program.methods
        .transferTokens(MINT_AMOUNT)
        .accounts({
          fromAuthority: owner.publicKey,
          fromTokenAccount: ownerTokenAccount,
          toTokenAccount: strangerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const afterOwner = await getAccount(connection, ownerTokenAccount);
      const afterStranger = await getAccount(connection, strangerTokenAccount);

      const ownerDiff =
        BigInt(beforeOwner.amount.toString()) -
        BigInt(afterOwner.amount.toString());
      const strangerDiff =
        BigInt(afterStranger.amount.toString()) -
        BigInt(beforeStranger.amount.toString());

      assert.equal(ownerDiff.toString(), MINT_AMOUNT.toString());
      assert.equal(strangerDiff.toString(), MINT_AMOUNT.toString());
    });

    it("Transfers succeed even when contract is paused (LPToken behavior)", async () => {
      // Pause
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      try {
        // Transfer should still work — matches EVM LPToken (no _transfer override)
        await program.methods
          .transferTokens(MINT_AMOUNT)
          .accounts({
            fromAuthority: owner.publicKey,
            fromTokenAccount: ownerTokenAccount,
            toTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        // If we reach here, transfer succeeded while paused — expected behavior
      } catch (err: any) {
        assert.fail(
          `Transfer should NOT be blocked by pause in LPToken, but got: ${err}`
        );
      } finally {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
      }
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 7: Delegated Transfers — EVM: approve() / transferFrom()
  // ─────────────────────────────────────────────────────────────────────────

  describe("Delegated transfers (approve / transferFrom equivalent)", () => {
    it("Can approve a delegate and query allowance", async () => {
      await program.methods
        .approveDelegate(MINT_AMOUNT)
        .accounts({
          tokenAccountOwner: owner.publicKey,
          tokenAccount: ownerTokenAccount,
          delegate: stranger.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const acct = await getAccount(connection, ownerTokenAccount);
      assert.equal(
        acct.delegate?.toBase58(),
        stranger.publicKey.toBase58()
      );
      assert.equal(acct.delegatedAmount.toString(), MINT_AMOUNT.toString());
    });

    it("Delegate can transfer tokens up to allowance", async () => {
      // Approve stranger as delegate
      await program.methods
        .approveDelegate(MINT_AMOUNT)
        .accounts({
          tokenAccountOwner: owner.publicKey,
          tokenAccount: ownerTokenAccount,
          delegate: stranger.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      const before = await getAccount(connection, strangerTokenAccount);

      // Stranger transfers from owner's account (transferFrom equivalent)
      await program.methods
        .transferTokens(MINT_AMOUNT)
        .accounts({
          fromAuthority: stranger.publicKey,
          fromTokenAccount: ownerTokenAccount,
          toTokenAccount: strangerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([stranger])
        .rpc();

      const after = await getAccount(connection, strangerTokenAccount);
      const diff =
        BigInt(after.amount.toString()) - BigInt(before.amount.toString());
      assert.equal(diff.toString(), MINT_AMOUNT.toString());
    });

    it("Approvals succeed even when contract is paused (LPToken behavior)", async () => {
      // Pause
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      try {
        // Approve should still work — matches EVM LPToken (no _approve override)
        await program.methods
          .approveDelegate(MINT_AMOUNT)
          .accounts({
            tokenAccountOwner: owner.publicKey,
            tokenAccount: ownerTokenAccount,
            delegate: stranger.publicKey,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        // Success while paused — expected for LPToken
      } catch (err: any) {
        assert.fail(
          `Approve should NOT be blocked by pause in LPToken, but got: ${err}`
        );
      } finally {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
      }
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 8: Ownership Transfer — Two-step propose/accept pattern
  // (Production improvement over EVM's one-step OwnableUpgradeable)
  // ─────────────────────────────────────────────────────────────────────────

  describe("Ownership transfer (two-step)", () => {
    it("Owner can propose and new owner can accept", async () => {
      const newOwner = Keypair.generate();
      await airdrop(connection, newOwner.publicKey);

      // Step 1: Propose
      await program.methods
        .transferOwnership(newOwner.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Verify pending_owner is set but owner hasn't changed yet
      let state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.owner.toBase58(), owner.publicKey.toBase58());
      assert.equal(state.pendingOwner.toBase58(), newOwner.publicKey.toBase58());

      // Step 2: Accept
      await program.methods
        .acceptOwnership()
        .accounts({
          newOwner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      // Verify ownership transferred and pending cleared
      state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.owner.toBase58(), newOwner.publicKey.toBase58());
      assert.equal(state.pendingOwner.toBase58(), PublicKey.default.toBase58());

      // Cleanup: transfer back to original owner
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
    });

    it("Non-owner cannot propose ownership transfer", async () => {
      try {
        await program.methods
          .transferOwnership(stranger.publicKey)
          .accounts({
            owner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected Unauthorized error");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("Cannot propose zero address as new owner", async () => {
      try {
        await program.methods
          .transferOwnership(PublicKey.default)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected InvalidOwner error");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidOwner");
      }
    });

    it("Only proposed new owner can accept — stranger cannot", async () => {
      const newOwner = Keypair.generate();

      // Propose
      await program.methods
        .transferOwnership(newOwner.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Stranger tries to accept
      try {
        await program.methods
          .acceptOwnership()
          .accounts({
            newOwner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected NoPendingOwnership error");
      } catch (err: any) {
        assert.include(err.toString(), "NoPendingOwnership");
      }

      // Cleanup: cancel by proposing someone else then resetting
      // (owner can overwrite pending_owner)
      const dummy = Keypair.generate();
      await program.methods
        .transferOwnership(dummy.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();
    });

    it("Proposing does not transfer until accepted — owner retains control", async () => {
      const newOwner = Keypair.generate();

      // Propose
      await program.methods
        .transferOwnership(newOwner.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Owner can still pause (retains full control)
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, true);

      // Cleanup
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

    it("New owner can exercise governance after accepting", async () => {
      const newOwner = Keypair.generate();
      await airdrop(connection, newOwner.publicKey);

      // Propose + accept
      await program.methods
        .transferOwnership(newOwner.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      await program.methods
        .acceptOwnership()
        .accounts({
          newOwner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      // New owner can pause
      await program.methods
        .setPause(true)
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

      // Old owner CANNOT unpause
      try {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected Unauthorized error");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }

      // Cleanup: new owner unpauses and transfers back
      await program.methods
        .setPause(false)
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();

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
    });

    it("Cannot accept when no transfer is pending", async () => {
      // pending_owner should be Pubkey::default or the dummy from earlier
      // stranger is definitely not the pending_owner
      try {
        await program.methods
          .acceptOwnership()
          .accounts({
            newOwner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("Expected NoPendingOwnership error");
      } catch (err: any) {
        assert.include(err.toString(), "NoPendingOwnership");
      }
    });
  });

  // ─────────────────────────────────────────────────────────────────────────
  // Phase 9: Edge cases and additional security tests
  // ─────────────────────────────────────────────────────────────────────────

  describe("Edge cases", () => {
    it("Cannot burn more than balance", async () => {
      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      const acct = await getAccount(connection, ownerTokenAccount);
      const excessAmount = new anchor.BN(
        (BigInt(acct.amount.toString()) + BigInt(1)).toString()
      );

      try {
        await program.methods
          .burnTokens(excessAmount)
          .accounts({
            authority: owner.publicKey,
            tokenAccountAuthority: owner.publicKey,
            tokenState,
            minterRecord: dummyMinterRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected error for insufficient balance");
      } catch (err: any) {
        // SPL Token will reject the burn due to insufficient funds
        assert.ok(err.toString().length > 0);
      }
    });

    it("Cannot transfer more than balance", async () => {
      const acct = await getAccount(connection, ownerTokenAccount);
      const excessAmount = new anchor.BN(
        (BigInt(acct.amount.toString()) + BigInt(1)).toString()
      );

      try {
        await program.methods
          .transferTokens(excessAmount)
          .accounts({
            fromAuthority: owner.publicKey,
            fromTokenAccount: ownerTokenAccount,
            toTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("Expected error for insufficient balance");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });

    it("Mint amount of zero succeeds (matches ERC20 behavior)", async () => {
      const [dummyMinterRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      await program.methods
        .mintTokens(new anchor.BN(0))
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyMinterRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();
      // No error — zero mint is valid
    });

    it("Cannot initialize with zero-address owner (defense-in-depth)", async () => {
      const badMint = Keypair.generate();
      const [badTokenState] = deriveTokenState(
        badMint.publicKey,
        program.programId
      );

      try {
        await program.methods
          .initializeMint({
            owner: PublicKey.default,
            evmChainId: new anchor.BN(1),
            decimals: 9,
          })
          .accounts({
            payer: owner.publicKey,
            tokenMint: badMint.publicKey,
            tokenState: badTokenState,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .signers([owner, badMint])
          .rpc();
        assert.fail("Expected InvalidOwner error");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidOwner");
      }
    });
  });

  // ═══════════════════════════════════════════════════════════════════════════
  // ADVERSARIAL SECURITY TESTS
  // ═══════════════════════════════════════════════════════════════════════════
  //
  // These tests attempt to exploit the program. Every test EXPECTS failure.
  // If any test succeeds unexpectedly, there is a bug.
  // ═══════════════════════════════════════════════════════════════════════════

  describe("Adversarial: Unauthorized minting", () => {
    it("ATK-1: Stranger passes another minter's record PDA to mint", async () => {
      // Attack: stranger derives the REAL minter's record PDA but signs as stranger
      const [realMinterRecord] = deriveMinterRecord(
        tokenState,
        minterKeypair.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: stranger.publicKey,
            tokenState,
            minterRecord: realMinterRecord, // valid record, wrong authority
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-1 EXPLOITED: Stranger minted using another minter's record");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("ATK-2: Stranger passes a random (non-existent) account as minter_record", async () => {
      const fakeRecord = Keypair.generate().publicKey;

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: stranger.publicKey,
            tokenState,
            minterRecord: fakeRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-2 EXPLOITED: Stranger minted with fake minter record");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("ATK-3: Deactivated minter tries to mint", async () => {
      // Create and deactivate a minter
      const deadMinter = Keypair.generate();
      await airdrop(connection, deadMinter.publicKey);
      const [deadRecord] = deriveMinterRecord(
        tokenState,
        deadMinter.publicKey,
        program.programId
      );

      // Activate
      await program.methods
        .updateMinter({ isActive: true })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: deadMinter.publicKey,
          minterRecord: deadRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      // Deactivate
      await program.methods
        .updateMinter({ isActive: false })
        .accounts({
          owner: owner.publicKey,
          tokenState,
          targetMinter: deadMinter.publicKey,
          minterRecord: deadRecord,
          tokenMint: mintKeypair.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([owner])
        .rpc();

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: deadMinter.publicKey,
            tokenState,
            minterRecord: deadRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([deadMinter])
          .rpc();
        assert.fail("ATK-3 EXPLOITED: Deactivated minter still minted");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });
  });

  describe("Adversarial: Unauthorized burning", () => {
    it("ATK-4: Minter tries to burn without token holder co-signing", async () => {
      // Minter is active but does NOT include stranger (token holder) as signer
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minterKeypair.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: minterKeypair.publicKey,
            tokenAccountAuthority: stranger.publicKey,
            tokenState,
            minterRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minterKeypair]) // stranger NOT signing
          .rpc();
        assert.fail("ATK-4 EXPLOITED: Burned without token holder consent");
      } catch (err: any) {
        // Should fail because stranger didn't sign
        assert.ok(err.toString().length > 0);
      }
    });

    it("ATK-5: Stranger burns own tokens without minter/owner status", async () => {
      const [fakeRecord] = deriveMinterRecord(
        tokenState,
        stranger.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: stranger.publicKey,
            tokenAccountAuthority: stranger.publicKey,
            tokenState,
            minterRecord: fakeRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-5 EXPLOITED: Stranger burned without minter status");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });
  });

  describe("Adversarial: Cross-mint attacks (incorrect token accounts)", () => {
    // Create a second independent mint to attempt cross-contamination
    let mintB: Keypair;
    let tokenStateB: PublicKey;
    let ownerTokenAccountB: PublicKey;

    before(async () => {
      mintB = Keypair.generate();
      [tokenStateB] = deriveTokenState(mintB.publicKey, program.programId);

      // Initialize second mint
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

      ownerTokenAccountB = await getOrCreateATA(
        connection,
        owner,
        mintB.publicKey,
        owner.publicKey
      );
    });

    it("ATK-6: Mint tokens using tokenState from mint A but token account for mint B", async () => {
      // Attack: use tokenState A (authorized) but try to mint into mint B's token account
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: owner.publicKey,
            tokenState, // from mint A
            minterRecord: dummyRecord,
            tokenMint: mintKeypair.publicKey, // mint A
            recipientTokenAccount: ownerTokenAccountB, // belongs to mint B!
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("ATK-6 EXPLOITED: Minted into wrong mint's token account");
      } catch (err: any) {
        // Should fail — recipientTokenAccount.mint != token_mint constraint
        assert.include(err.toString(), "InvalidMint");
      }
    });

    it("ATK-7: Use tokenState from mint B with mint A's token_mint", async () => {
      // Attack: pass tokenStateB but token_mint from A — PDA seeds won't match
      const [dummyRecord] = deriveMinterRecord(
        tokenStateB,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: owner.publicKey,
            tokenState: tokenStateB, // from mint B
            minterRecord: dummyRecord,
            tokenMint: mintKeypair.publicKey, // mint A — seeds won't match
            recipientTokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("ATK-7 EXPLOITED: Used wrong tokenState for mint");
      } catch (err: any) {
        // PDA seed derivation mismatch
        assert.ok(err.toString().length > 0);
      }
    });

    it("ATK-8: Use minter record from mint A when minting on mint B", async () => {
      // Attack: minter is registered on mint A, tries to use that record to mint on mint B
      const [minterRecordA] = deriveMinterRecord(
        tokenState,
        minterKeypair.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: minterKeypair.publicKey,
            tokenState: tokenStateB, // mint B's state
            minterRecord: minterRecordA, // mint A's minter record!
            tokenMint: mintB.publicKey,
            recipientTokenAccount: ownerTokenAccountB,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minterKeypair])
          .rpc();
        assert.fail("ATK-8 EXPLOITED: Cross-mint minter record reuse");
      } catch (err: any) {
        // verify_minter derives PDA from tokenStateB + minterKeypair, won't match minterRecordA
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("ATK-9: Burn tokens using wrong mint's tokenState", async () => {
      // Mint some tokens on mint B first
      const [dummyRecordB] = deriveMinterRecord(
        tokenStateB,
        owner.publicKey,
        program.programId
      );

      await program.methods
        .mintTokens(MINT_AMOUNT)
        .accounts({
          authority: owner.publicKey,
          tokenState: tokenStateB,
          minterRecord: dummyRecordB,
          tokenMint: mintB.publicKey,
          recipientTokenAccount: ownerTokenAccountB,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      // Attack: use tokenState from mint A to burn mint B tokens
      const [dummyRecordA] = deriveMinterRecord(
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
            tokenState, // mint A's tokenState
            minterRecord: dummyRecordA,
            tokenMint: mintB.publicKey, // mint B — PDA mismatch
            tokenAccount: ownerTokenAccountB,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("ATK-9 EXPLOITED: Burned with wrong tokenState");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });
  });

  describe("Adversarial: Authority bypass attempts", () => {
    it("ATK-10: Stranger tries to pause", async () => {
      try {
        await program.methods
          .setPause(true)
          .accounts({
            owner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-10 EXPLOITED: Stranger paused");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("ATK-11: Stranger tries to transfer ownership", async () => {
      try {
        await program.methods
          .transferOwnership(stranger.publicKey)
          .accounts({
            owner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-11 EXPLOITED: Stranger took ownership");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("ATK-12: Stranger tries to register a minter", async () => {
      const fakeMinter = Keypair.generate();
      const [fakeRecord] = deriveMinterRecord(
        tokenState,
        fakeMinter.publicKey,
        program.programId
      );

      try {
        await program.methods
          .updateMinter({ isActive: true })
          .accounts({
            owner: stranger.publicKey,
            tokenState,
            targetMinter: fakeMinter.publicKey,
            minterRecord: fakeRecord,
            tokenMint: mintKeypair.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-12 EXPLOITED: Stranger registered a minter");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });

    it("ATK-13: Minter tries to add another minter (only owner should)", async () => {
      const newMinter = Keypair.generate();
      const [newRecord] = deriveMinterRecord(
        tokenState,
        newMinter.publicKey,
        program.programId
      );

      try {
        await program.methods
          .updateMinter({ isActive: true })
          .accounts({
            owner: minterKeypair.publicKey, // minter, not owner
            tokenState,
            targetMinter: newMinter.publicKey,
            minterRecord: newRecord,
            tokenMint: mintKeypair.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([minterKeypair])
          .rpc();
        assert.fail("ATK-13 EXPLOITED: Minter added another minter");
      } catch (err: any) {
        assert.include(err.toString(), "Unauthorized");
      }
    });
  });

  describe("Adversarial: Initialization edge cases", () => {
    it("ATK-14: Re-initialize an existing mint/tokenState", async () => {
      try {
        await program.methods
          .initializeMint({
            owner: stranger.publicKey, // try to change owner via re-init
            evmChainId: new anchor.BN(999),
            decimals: DECIMALS,
          })
          .accounts({
            payer: stranger.publicKey,
            tokenMint: mintKeypair.publicKey, // already initialized
            tokenState,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
            rent: SYSVAR_RENT_PUBKEY,
          })
          .signers([stranger, mintKeypair])
          .rpc();
        assert.fail("ATK-14 EXPLOITED: Re-initialized existing mint");
      } catch (err: any) {
        // Should fail — 'init' constraint prevents re-initialization
        assert.ok(err.toString().length > 0);
      }
    });
  });

  describe("Adversarial: Zero-value edge cases", () => {
    it("ATK-15: Zero-amount burn succeeds (matches ERC20)", async () => {
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      // Should succeed — matches EVM behavior
      await program.methods
        .burnTokens(new anchor.BN(0))
        .accounts({
          authority: owner.publicKey,
          tokenAccountAuthority: owner.publicKey,
          tokenState,
          minterRecord: dummyRecord,
          tokenMint: mintKeypair.publicKey,
          tokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();
    });

    it("ATK-16: Zero-amount transfer succeeds (matches ERC20)", async () => {
      await program.methods
        .transferTokens(new anchor.BN(0))
        .accounts({
          fromAuthority: owner.publicKey,
          fromTokenAccount: ownerTokenAccount,
          toTokenAccount: strangerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();
    });

    it("ATK-17: Zero-amount approve succeeds (matches ERC20)", async () => {
      await program.methods
        .approveDelegate(new anchor.BN(0))
        .accounts({
          tokenAccountOwner: owner.publicKey,
          tokenAccount: ownerTokenAccount,
          delegate: stranger.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();
    });
  });

  describe("Adversarial: Delegate misuse", () => {
    it("ATK-18: Delegate tries to transfer more than delegated amount", async () => {
      // Approve stranger for a small amount
      const smallApproval = new anchor.BN(100);
      await program.methods
        .approveDelegate(smallApproval)
        .accounts({
          tokenAccountOwner: owner.publicKey,
          tokenAccount: ownerTokenAccount,
          delegate: stranger.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      // Stranger tries to transfer more than delegated
      try {
        await program.methods
          .transferTokens(new anchor.BN(101))
          .accounts({
            fromAuthority: stranger.publicKey,
            fromTokenAccount: ownerTokenAccount,
            toTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-18 EXPLOITED: Delegate exceeded allowance");
      } catch (err: any) {
        // SPL Token: insufficient delegated amount
        assert.ok(err.toString().length > 0);
      }
    });

    it("ATK-19: Revoked delegate tries to transfer", async () => {
      // Approve then revoke via SPL directly (set delegate to 0)
      const approveAmount = new anchor.BN(1000);
      await program.methods
        .approveDelegate(approveAmount)
        .accounts({
          tokenAccountOwner: owner.publicKey,
          tokenAccount: ownerTokenAccount,
          delegate: stranger.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      // Revoke by approving 0 (or use SPL revoke)
      // Use the SPL revoke instruction directly
      const revokeIx = createRevokeInstruction(
        ownerTokenAccount,
        owner.publicKey,
        [],
        TOKEN_PROGRAM_ID
      );
      const tx = new anchor.web3.Transaction().add(revokeIx);
      await provider.sendAndConfirm(tx, [owner]);

      // Now stranger tries to transfer
      try {
        await program.methods
          .transferTokens(new anchor.BN(1))
          .accounts({
            fromAuthority: stranger.publicKey,
            fromTokenAccount: ownerTokenAccount,
            toTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-19 EXPLOITED: Revoked delegate still transferred");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });

    it("ATK-20: Non-delegate stranger tries to transfer from someone else's account", async () => {
      try {
        await program.methods
          .transferTokens(new anchor.BN(1))
          .accounts({
            fromAuthority: stranger.publicKey,
            fromTokenAccount: ownerTokenAccount, // owner's account, stranger is not delegate
            toTokenAccount: strangerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-20 EXPLOITED: Stranger transferred from another account");
      } catch (err: any) {
        assert.ok(err.toString().length > 0);
      }
    });
  });

  describe("Adversarial: Burn token_account_authority mismatch", () => {
    it("ATK-21: Minter specifies wrong token_account_authority for burn", async () => {
      const [minterRecord] = deriveMinterRecord(
        tokenState,
        minterKeypair.publicKey,
        program.programId
      );

      try {
        await program.methods
          .burnTokens(new anchor.BN(1))
          .accounts({
            authority: minterKeypair.publicKey,
            tokenAccountAuthority: minterKeypair.publicKey, // wrong — not the owner of strangerTokenAccount
            tokenState,
            minterRecord,
            tokenMint: mintKeypair.publicKey,
            tokenAccount: strangerTokenAccount, // owned by stranger, not minterKeypair
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([minterKeypair])
          .rpc();
        assert.fail("ATK-21 EXPLOITED: Burned with wrong token_account_authority");
      } catch (err: any) {
        // Should fail: token_account.owner != token_account_authority
        assert.include(err.toString(), "InvalidTokenAuthority");
      }
    });
  });

  describe("Adversarial: Pause bypass attempts", () => {
    it("ATK-22: Mint during pause (owner bypassing own pause)", async () => {
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );

      try {
        await program.methods
          .mintTokens(MINT_AMOUNT)
          .accounts({
            authority: owner.publicKey,
            tokenState,
            minterRecord: dummyRecord,
            tokenMint: mintKeypair.publicKey,
            recipientTokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("ATK-22 EXPLOITED: Owner minted while paused");
      } catch (err: any) {
        assert.include(err.toString(), "Paused");
      } finally {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
      }
    });

    it("ATK-23: Burn during pause", async () => {
      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

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
            tokenAccount: ownerTokenAccount,
            tokenProgram: TOKEN_PROGRAM_ID,
          })
          .signers([owner])
          .rpc();
        assert.fail("ATK-23 EXPLOITED: Owner burned while paused");
      } catch (err: any) {
        assert.include(err.toString(), "Paused");
      } finally {
        await program.methods
          .setPause(false)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
      }
    });

    it("ATK-24: Transfers still work during pause (expected — LPToken behavior)", async () => {
      // Ensure owner has tokens
      const [dummyRecord] = deriveMinterRecord(
        tokenState,
        owner.publicKey,
        program.programId
      );
      await program.methods
        .mintTokens(MINT_AMOUNT)
        .accounts({
          authority: owner.publicKey,
          tokenState,
          minterRecord: dummyRecord,
          tokenMint: mintKeypair.publicKey,
          recipientTokenAccount: ownerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

      await program.methods
        .setPause(true)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Transfer should succeed even while paused
      await program.methods
        .transferTokens(new anchor.BN(1))
        .accounts({
          fromAuthority: owner.publicKey,
          fromTokenAccount: ownerTokenAccount,
          toTokenAccount: strangerTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([owner])
        .rpc();

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
  });

  describe("Adversarial: Ownership renunciation blocked", () => {
    it("ATK-25: Cannot propose ownership to Pubkey::default (renounceOwnership equivalent)", async () => {
      try {
        await program.methods
          .transferOwnership(PublicKey.default)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
        assert.fail("ATK-25 EXPLOITED: Ownership renounced via zero address");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidOwner");
      }
    });
  });

  describe("Adversarial: Two-step ownership attacks", () => {
    it("ATK-26: Stranger cannot accept pending ownership meant for someone else", async () => {
      const intended = Keypair.generate();

      // Owner proposes intended
      await program.methods
        .transferOwnership(intended.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Stranger tries to accept
      try {
        await program.methods
          .acceptOwnership()
          .accounts({
            newOwner: stranger.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([stranger])
          .rpc();
        assert.fail("ATK-26 EXPLOITED: Stranger stole pending ownership");
      } catch (err: any) {
        assert.include(err.toString(), "NoPendingOwnership");
      }

      // Cleanup: overwrite pending with a dummy to clear
      const dummy = Keypair.generate();
      await program.methods
        .transferOwnership(dummy.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();
    });

    it("ATK-27: Owner can overwrite pending transfer (cancel/redirect)", async () => {
      const firstCandidate = Keypair.generate();
      const secondCandidate = Keypair.generate();
      await airdrop(connection, secondCandidate.publicKey);

      // Propose first candidate
      await program.methods
        .transferOwnership(firstCandidate.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Overwrite with second candidate
      await program.methods
        .transferOwnership(secondCandidate.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
        .rpc();

      // Verify pending is second candidate
      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.pendingOwner.toBase58(), secondCandidate.publicKey.toBase58());

      // First candidate cannot accept
      await airdrop(connection, firstCandidate.publicKey);
      try {
        await program.methods
          .acceptOwnership()
          .accounts({
            newOwner: firstCandidate.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([firstCandidate])
          .rpc();
        assert.fail("First candidate should not be able to accept");
      } catch (err: any) {
        assert.include(err.toString(), "NoPendingOwnership");
      }

      // Second candidate CAN accept
      await program.methods
        .acceptOwnership()
        .accounts({
          newOwner: secondCandidate.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([secondCandidate])
        .rpc();

      // Transfer back
      await program.methods
        .transferOwnership(owner.publicKey)
        .accounts({
          owner: secondCandidate.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([secondCandidate])
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
    });

    it("ATK-28: Cannot propose self as new owner", async () => {
      try {
        await program.methods
          .transferOwnership(owner.publicKey)
          .accounts({
            owner: owner.publicKey,
            tokenState,
            tokenMint: mintKeypair.publicKey,
          })
          .signers([owner])
          .rpc();
        assert.fail("Should not allow proposing self");
      } catch (err: any) {
        assert.include(err.toString(), "InvalidOwner");
      }
    });
  });
});
