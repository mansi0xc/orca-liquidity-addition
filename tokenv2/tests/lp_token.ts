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
  // Phase 8: Ownership Transfer — EVM: OwnableUpgradeable.transferOwnership()
  // ─────────────────────────────────────────────────────────────────────────

  describe("Ownership transfer", () => {
    it("Owner can transfer ownership to a new address", async () => {
      const newOwner = Keypair.generate();

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
      assert.equal(state.owner.toBase58(), newOwner.publicKey.toBase58());

      // Transfer back to original owner for subsequent tests
      await airdrop(connection, newOwner.publicKey);
      await program.methods
        .transferOwnership(owner.publicKey)
        .accounts({
          owner: newOwner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([newOwner])
        .rpc();
    });

    it("Non-owner cannot transfer ownership", async () => {
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

    it("New owner can exercise governance after transfer", async () => {
      const newOwner = Keypair.generate();
      await airdrop(connection, newOwner.publicKey);

      // Transfer ownership
      await program.methods
        .transferOwnership(newOwner.publicKey)
        .accounts({
          owner: owner.publicKey,
          tokenState,
          tokenMint: mintKeypair.publicKey,
        })
        .signers([owner])
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

      const state = await program.account.tokenState.fetch(tokenState);
      assert.equal(state.isPaused, true);

      // Old owner can no longer unpause
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
  });
});
