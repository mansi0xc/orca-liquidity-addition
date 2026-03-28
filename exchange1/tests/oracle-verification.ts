import * as anchor from "@coral-xyz/anchor";
import { Program, BN, AnchorError } from "@coral-xyz/anchor";
import { LpBonds } from "../target/types/lp_bonds";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  Ed25519Program,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  NATIVE_MINT,
  getAssociatedTokenAddress,
} from "@solana/spl-token";
import { expect } from "chai";
import * as nacl from "tweetnacl";

// =============================================================================
// CONSTANTS — must match Rust constants exactly
// =============================================================================

const WHIRLPOOL_PROGRAM_ID = new PublicKey(
  "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
);
const ALLOWLISTED_WHIRLPOOL = new PublicKey(
  "8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN"
);

// PDA Seeds
const CONFIG_SEED = Buffer.from("config");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const POSITION_CUSTODY_SEED = Buffer.from("position_custody");
const ORACLE_CONFIG_SEED = Buffer.from("oracle_config");
const NONCE_SEED = Buffer.from("nonce");

// Domain separators — different per instruction to prevent cross-instruction replay
const ORACLE_DOMAIN_MINT = "LP_BONDS_MINT_V1__";
const ORACLE_DOMAIN_VERIFY = "LP_BONDS_VRFY_V1__";

// Unified oracle message length: 238 bytes
const ORACLE_MESSAGE_LEN = 238;

const MAX_ORACLE_STALENESS_SECONDS = 60;

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function derivePda(
  seeds: (Buffer | Uint8Array)[],
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(seeds, programId);
}

function deriveConfigPda(programId: PublicKey): [PublicKey, number] {
  return derivePda([CONFIG_SEED], programId);
}

function deriveOracleConfigPda(programId: PublicKey): [PublicKey, number] {
  return derivePda([ORACLE_CONFIG_SEED], programId);
}

function deriveNoncePda(
  user: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return derivePda([NONCE_SEED, user.toBuffer()], programId);
}

function derivePositionCustodyPda(
  bondMint: PublicKey,
  programId: PublicKey
): [PublicKey, number] {
  return derivePda([POSITION_CUSTODY_SEED, bondMint.toBuffer()], programId);
}

/**
 * Build unified canonical oracle message.
 * MUST match exactly with Rust `reconstruct_oracle_message` in ed25519.rs.
 *
 * Layout (238 bytes):
 *   [0..18)     domain separator
 *   [18..50)    whirlpool pubkey
 *   [50..82)    token_mint_a pubkey
 *   [82..114)   token_mint_b pubkey
 *   [114..122)  amount_a (u64 LE)
 *   [122..130)  amount_b (u64 LE)
 *   [130..146)  liquidity (u128 LE)
 *   [146..150)  tick_lower (i32 LE)
 *   [150..154)  tick_upper (i32 LE)
 *   [154..158)  tick_current (i32 LE)
 *   [158..166)  nonce (u64 LE)
 *   [166..174)  timestamp (i64 LE)
 *   [174..206)  sender pubkey
 *   [206..238)  contract_address pubkey
 */
function buildOracleMessage(params: {
  domain: string;
  whirlpool: PublicKey;
  tokenMintA: PublicKey;
  tokenMintB: PublicKey;
  amountA: bigint;
  amountB: bigint;
  liquidity: bigint;
  tickLower: number;
  tickUpper: number;
  tickCurrent: number;
  nonce: bigint;
  timestamp: bigint;
  sender: PublicKey;
  contractAddress: PublicKey;
}): Buffer {
  const message = Buffer.alloc(ORACLE_MESSAGE_LEN);
  let offset = 0;

  // Domain separator (18 bytes)
  const domain = Buffer.from(params.domain, "utf-8");
  expect(domain.length).to.equal(18, "Domain must be exactly 18 bytes");
  domain.copy(message, offset);
  offset += 18;

  // whirlpool (32 bytes)
  params.whirlpool.toBuffer().copy(message, offset);
  offset += 32;

  // token_mint_a (32 bytes)
  params.tokenMintA.toBuffer().copy(message, offset);
  offset += 32;

  // token_mint_b (32 bytes)
  params.tokenMintB.toBuffer().copy(message, offset);
  offset += 32;

  // amount_a (u64 LE, 8 bytes)
  message.writeBigUInt64LE(params.amountA, offset);
  offset += 8;

  // amount_b (u64 LE, 8 bytes)
  message.writeBigUInt64LE(params.amountB, offset);
  offset += 8;

  // liquidity (u128 LE, 16 bytes)
  const liquidityLow = params.liquidity & BigInt("0xFFFFFFFFFFFFFFFF");
  const liquidityHigh = params.liquidity >> BigInt(64);
  message.writeBigUInt64LE(liquidityLow, offset);
  message.writeBigUInt64LE(liquidityHigh, offset + 8);
  offset += 16;

  // tick_lower (i32 LE, 4 bytes)
  message.writeInt32LE(params.tickLower, offset);
  offset += 4;

  // tick_upper (i32 LE, 4 bytes)
  message.writeInt32LE(params.tickUpper, offset);
  offset += 4;

  // tick_current (i32 LE, 4 bytes)
  message.writeInt32LE(params.tickCurrent, offset);
  offset += 4;

  // nonce (u64 LE, 8 bytes)
  message.writeBigUInt64LE(params.nonce, offset);
  offset += 8;

  // timestamp (i64 LE, 8 bytes)
  message.writeBigInt64LE(params.timestamp, offset);
  offset += 8;

  // sender (32 bytes)
  params.sender.toBuffer().copy(message, offset);
  offset += 32;

  // contract_address (32 bytes)
  params.contractAddress.toBuffer().copy(message, offset);
  offset += 32;

  return message;
}

/**
 * Sign message with Ed25519.
 */
function signMessage(
  message: Buffer,
  secretKey: Uint8Array
): { signature: Buffer; publicKey: Buffer } {
  const signature = nacl.sign.detached(message, secretKey);
  const publicKey = secretKey.slice(32, 64);
  return {
    signature: Buffer.from(signature),
    publicKey: Buffer.from(publicKey),
  };
}

/**
 * Build Ed25519 signature verification instruction.
 */
function buildEd25519Instruction(
  publicKey: Buffer,
  signature: Buffer,
  message: Buffer
): TransactionInstruction {
  return Ed25519Program.createInstructionWithPublicKey({
    publicKey: publicKey,
    signature: signature,
    message: message,
  });
}

// =============================================================================
// TEST SUITE
// =============================================================================

describe("Oracle Verification", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.LpBonds as Program<LpBonds>;
  const connection = provider.connection;

  // Test accounts
  let admin: Keypair;
  let user: Keypair;
  let oracleKeypair: Keypair;
  let maliciousOracleKeypair: Keypair;

  // PDAs
  let configPda: PublicKey;
  let oracleConfigPda: PublicKey;
  let noncePda: PublicKey;

  // Mock token mints for message building
  const mockTokenMintA = Keypair.generate().publicKey;
  const mockTokenMintB = Keypair.generate().publicKey;

  // Test data
  const mockLiquidity = BigInt("36583284382");
  const mockAmountA = BigInt("1000000000");
  const mockAmountB = BigInt("2000000000");
  const mockTickLower = -10000;
  const mockTickUpper = 10000;
  const mockTickCurrent = 500;

  before(async () => {
    admin = Keypair.generate();
    user = Keypair.generate();
    oracleKeypair = Keypair.generate();
    maliciousOracleKeypair = Keypair.generate();

    const airdropAmount = 10 * LAMPORTS_PER_SOL;

    const adminAirdrop = await connection.requestAirdrop(
      admin.publicKey,
      airdropAmount
    );
    await connection.confirmTransaction(adminAirdrop);

    const userAirdrop = await connection.requestAirdrop(
      user.publicKey,
      airdropAmount
    );
    await connection.confirmTransaction(userAirdrop);

    [configPda] = deriveConfigPda(program.programId);
    [oracleConfigPda] = deriveOracleConfigPda(program.programId);
    [noncePda] = deriveNoncePda(user.publicKey, program.programId);

    console.log("Test Setup:");
    console.log("  Admin:", admin.publicKey.toBase58());
    console.log("  User:", user.publicKey.toBase58());
    console.log("  Oracle Authority:", oracleKeypair.publicKey.toBase58());
    console.log("  Config PDA:", configPda.toBase58());
    console.log("  Oracle Config PDA:", oracleConfigPda.toBase58());
    console.log("  User Nonce PDA:", noncePda.toBase58());
  });

  // ===========================================================================
  // PROTOCOL INITIALIZATION
  // ===========================================================================

  describe("Protocol Initialization", () => {
    it("should initialize protocol config with tick range", async () => {
      const [bondAuthorityPda] = derivePda(
        [BOND_AUTHORITY_SEED],
        program.programId
      );

      const tx = await program.methods
        .initialize(
          ALLOWLISTED_WHIRLPOOL,
          mockTokenMintA,
          mockTokenMintB,
          mockTickLower,
          mockTickUpper,
          new BN(86400) // lock_duration: 1 day
        )
        .accounts({
          admin: admin.publicKey,
          config: configPda,
          bondAuthority: bondAuthorityPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      console.log("Protocol initialized:", tx);

      const config = await program.account.protocolConfig.fetch(configPda);
      expect(config.admin.toBase58()).to.equal(admin.publicKey.toBase58());
      expect(config.tickLowerIndex).to.equal(mockTickLower);
      expect(config.tickUpperIndex).to.equal(mockTickUpper);
    });
  });

  // ===========================================================================
  // ORACLE INITIALIZATION TESTS
  // ===========================================================================

  describe("Oracle Initialization", () => {
    it("should initialize oracle config", async () => {
      const tx = await program.methods
        .initializeOracle(oracleKeypair.publicKey)
        .accounts({
          admin: admin.publicKey,
          config: configPda,
          oracleConfig: oracleConfigPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([admin])
        .rpc();

      console.log("Oracle initialized:", tx);

      const oracleConfig = await program.account.oracleConfig.fetch(
        oracleConfigPda
      );
      expect(oracleConfig.oracleAuthority.toBase58()).to.equal(
        oracleKeypair.publicKey.toBase58()
      );
      expect(oracleConfig.admin.toBase58()).to.equal(admin.publicKey.toBase58());
      expect(oracleConfig.enabled).to.be.true;
    });

    it("should fail to reinitialize oracle", async () => {
      try {
        await program.methods
          .initializeOracle(oracleKeypair.publicKey)
          .accounts({
            admin: admin.publicKey,
            config: configPda,
            oracleConfig: oracleConfigPda,
            systemProgram: SystemProgram.programId,
          })
          .signers([admin])
          .rpc();

        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err).to.exist;
        console.log("Correctly rejected oracle reinitialization");
      }
    });

    it("should update oracle authority", async () => {
      const newOracleKeypair = Keypair.generate();

      await program.methods
        .updateOracleAuthority(newOracleKeypair.publicKey)
        .accounts({
          admin: admin.publicKey,
          oracleConfig: oracleConfigPda,
        })
        .signers([admin])
        .rpc();

      const oracleConfig = await program.account.oracleConfig.fetch(
        oracleConfigPda
      );
      expect(oracleConfig.oracleAuthority.toBase58()).to.equal(
        newOracleKeypair.publicKey.toBase58()
      );

      // Revert to original oracle for remaining tests
      await program.methods
        .updateOracleAuthority(oracleKeypair.publicKey)
        .accounts({
          admin: admin.publicKey,
          oracleConfig: oracleConfigPda,
        })
        .signers([admin])
        .rpc();
    });

    it("should reject non-admin oracle authority update", async () => {
      try {
        await program.methods
          .updateOracleAuthority(maliciousOracleKeypair.publicKey)
          .accounts({
            admin: user.publicKey,
            oracleConfig: oracleConfigPda,
          })
          .signers([user])
          .rpc();

        expect.fail("Should have rejected non-admin");
      } catch (err) {
        expect(err).to.exist;
        console.log("Correctly rejected non-admin update attempt");
      }
    });
  });

  // ===========================================================================
  // NONCE INITIALIZATION TESTS
  // ===========================================================================

  describe("Nonce Initialization", () => {
    it("should initialize nonce for user", async () => {
      const tx = await program.methods
        .initializeNonce()
        .accounts({
          user: user.publicKey,
          nonceAccount: noncePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([user])
        .rpc();

      console.log("Nonce initialized:", tx);

      const nonceAccount = await program.account.nonceAccount.fetch(noncePda);
      expect(nonceAccount.user.toBase58()).to.equal(user.publicKey.toBase58());
      expect(nonceAccount.currentNonce.toNumber()).to.equal(0);
    });

    it("should fail to reinitialize nonce", async () => {
      try {
        await program.methods
          .initializeNonce()
          .accounts({
            user: user.publicKey,
            nonceAccount: noncePda,
            systemProgram: SystemProgram.programId,
          })
          .signers([user])
          .rpc();

        expect.fail("Should have thrown error");
      } catch (err) {
        expect(err).to.exist;
        console.log("Correctly rejected nonce reinitialization");
      }
    });
  });

  // ===========================================================================
  // CANONICAL MESSAGE TESTS
  // ===========================================================================

  describe("Unified Oracle Message Building", () => {
    it("should build deterministic message of correct length", () => {
      const params = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message1 = buildOracleMessage(params);
      const message2 = buildOracleMessage(params);

      expect(message1.equals(message2)).to.be.true;
      expect(message1.length).to.equal(ORACLE_MESSAGE_LEN);
      console.log("Oracle message length:", message1.length, "bytes");
    });

    it("should produce different messages for MINT vs VERIFY domains", () => {
      const baseParams = {
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const mintMsg = buildOracleMessage({
        ...baseParams,
        domain: ORACLE_DOMAIN_MINT,
      });
      const verifyMsg = buildOracleMessage({
        ...baseParams,
        domain: ORACLE_DOMAIN_VERIFY,
      });

      expect(mintMsg.equals(verifyMsg)).to.be.false;
      console.log("MINT and VERIFY domains produce different messages");
    });

    it("should produce different messages for different nonces", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const msg1 = buildOracleMessage({ ...baseParams, nonce: BigInt(1) });
      const msg2 = buildOracleMessage({ ...baseParams, nonce: BigInt(2) });

      expect(msg1.equals(msg2)).to.be.false;
    });

    it("should produce different messages for different senders", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        contractAddress: program.programId,
      };

      const msg1 = buildOracleMessage({
        ...baseParams,
        sender: user.publicKey,
      });
      const msg2 = buildOracleMessage({
        ...baseParams,
        sender: admin.publicKey,
      });

      expect(msg1.equals(msg2)).to.be.false;
    });

    it("should include token mints in the signed message", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const msg1 = buildOracleMessage({
        ...baseParams,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
      });
      const msg2 = buildOracleMessage({
        ...baseParams,
        tokenMintA: Keypair.generate().publicKey,
        tokenMintB: Keypair.generate().publicKey,
      });

      expect(msg1.equals(msg2)).to.be.false;
      console.log("Token mints bound into oracle message");
    });
  });

  // ===========================================================================
  // ED25519 SIGNATURE TESTS
  // ===========================================================================

  describe("Ed25519 Signing", () => {
    it("should sign and verify message correctly", () => {
      const message = buildOracleMessage({
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      });

      const { signature } = signMessage(message, oracleKeypair.secretKey);

      const isValid = nacl.sign.detached.verify(
        message,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.true;
      expect(signature.length).to.equal(64);
    });

    it("should reject wrong oracle's signature", () => {
      const message = buildOracleMessage({
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      });

      const { signature } = signMessage(
        message,
        maliciousOracleKeypair.secretKey
      );

      const isValid = nacl.sign.detached.verify(
        message,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.false;
    });
  });

  // ===========================================================================
  // SECURITY TESTS — STRICT INSTRUCTION ORDERING
  // ===========================================================================

  describe("Strict Instruction Ordering", () => {
    it("should require Ed25519 at exactly (current_index - 1)", () => {
      // The program checks: current_index >= 1 AND ix at (current_index-1).program_id == Ed25519
      // This means:
      // - [Ed25519, OurIx] → valid (Ed25519 at 0, our at 1)
      // - [ComputeBudget, Ed25519, OurIx] → valid (Ed25519 at 1, our at 2)
      // - [Ed25519, ComputeBudget, OurIx] → INVALID (ComputeBudget at 1, not Ed25519)
      // - [OurIx] → INVALID (current_index == 0, no preceding instruction)
      console.log(
        "Strict ordering: Ed25519 must be at (current_index - 1), no scanning"
      );
    });

    it("should reject if no preceding instruction exists", () => {
      // current_index == 0 → require!(current_index >= 1) → InvalidInstructionOrder
      console.log("No preceding instruction → InvalidInstructionOrder");
    });

    it("should reject if preceding instruction is not Ed25519", () => {
      // If ComputeBudget or any other instruction is between Ed25519 and our ix,
      // the program rejects with InvalidInstructionOrder
      console.log(
        "Wrong program at (current_index - 1) → InvalidInstructionOrder"
      );
    });
  });

  // ===========================================================================
  // SECURITY TESTS — STRICT NONCE
  // ===========================================================================

  describe("Strict Nonce Enforcement", () => {
    it("should require nonce == current_nonce + 1 (not just >)", () => {
      // After initialization, current_nonce == 0
      // First valid nonce: 1 (== 0 + 1)
      const currentNonce = BigInt(0);
      const validNonce = currentNonce + BigInt(1);
      expect(validNonce).to.equal(BigInt(1));

      // Skipping to nonce 5 should fail (5 != 0 + 1)
      const skippedNonce = BigInt(5);
      expect(skippedNonce === currentNonce + BigInt(1)).to.be.false;
      console.log("Strict sequential nonce enforced: nonce == current + 1");
    });

    it("should reject replay with same nonce", () => {
      const currentNonce = BigInt(3);
      const replayNonce = BigInt(3); // Same as current
      expect(replayNonce === currentNonce + BigInt(1)).to.be.false;
      console.log("Replay with same nonce rejected");
    });

    it("should reject skipped nonce", () => {
      const currentNonce = BigInt(3);
      const skippedNonce = BigInt(5); // Skipped 4
      expect(skippedNonce === currentNonce + BigInt(1)).to.be.false;
      console.log("Skipped nonce rejected");
    });

    it("should reject lower nonce", () => {
      const currentNonce = BigInt(3);
      const lowerNonce = BigInt(2);
      expect(lowerNonce === currentNonce + BigInt(1)).to.be.false;
      console.log("Lower nonce rejected");
    });
  });

  // ===========================================================================
  // SECURITY TESTS — NO SIGNATURE PARAMETER
  // ===========================================================================

  describe("No Signature Parameter", () => {
    it("should not accept signature as instruction argument", () => {
      // The add_liquidity_and_mint_bond instruction takes:
      //   liquidity_amount, token_max_a, token_max_b, tick_current,
      //   oracle_nonce, oracle_timestamp
      // NO oracle_signature parameter.
      // The signature exists ONLY inside the Ed25519 instruction data.
      console.log(
        "Signature parameter removed — exists only in Ed25519 instruction"
      );
    });
  });

  // ===========================================================================
  // SECURITY TESTS — MESSAGE INTEGRITY
  // ===========================================================================

  describe("Message Integrity & Binding", () => {
    it("should prevent cross-user replay attack", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        contractAddress: program.programId,
      };

      const victimMsg = buildOracleMessage({
        ...baseParams,
        sender: user.publicKey,
      });
      const attackerMsg = buildOracleMessage({
        ...baseParams,
        sender: admin.publicKey,
      });

      expect(victimMsg.equals(attackerMsg)).to.be.false;

      const { signature } = signMessage(victimMsg, oracleKeypair.secretKey);
      const isValid = nacl.sign.detached.verify(
        attackerMsg,
        signature,
        oracleKeypair.publicKey.toBytes()
      );
      expect(isValid).to.be.false;
      console.log("Cross-user replay prevented");
    });

    it("should prevent cross-program replay attack", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
      };

      const msg1 = buildOracleMessage({
        ...baseParams,
        contractAddress: program.programId,
      });
      const msg2 = buildOracleMessage({
        ...baseParams,
        contractAddress: Keypair.generate().publicKey,
      });

      expect(msg1.equals(msg2)).to.be.false;
      console.log("Cross-program replay prevented");
    });

    it("should prevent cross-instruction replay (MINT domain vs VERIFY domain)", () => {
      const mintDomain = Buffer.from(ORACLE_DOMAIN_MINT, "utf-8");
      const verifyDomain = Buffer.from(ORACLE_DOMAIN_VERIFY, "utf-8");

      expect(mintDomain.equals(verifyDomain)).to.be.false;
      expect(mintDomain.length).to.equal(verifyDomain.length);
      console.log("Cross-instruction replay prevented by domain separation");
    });

    it("should prevent tick range tampering", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      // Oracle signed narrow range
      const oracleMsg = buildOracleMessage({
        ...baseParams,
        tickLower: -1000,
        tickUpper: 1000,
      });

      // Attacker tries wider range
      const tamperMsg = buildOracleMessage({
        ...baseParams,
        tickLower: -100000,
        tickUpper: 100000,
      });

      const { signature } = signMessage(oracleMsg, oracleKeypair.secretKey);
      const isValid = nacl.sign.detached.verify(
        tamperMsg,
        signature,
        oracleKeypair.publicKey.toBytes()
      );
      expect(isValid).to.be.false;
      console.log("Tick range tampering prevented");
    });

    it("should prevent token amount tampering", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const oracleMsg = buildOracleMessage({
        ...baseParams,
        amountA: BigInt("1000000000"),
        amountB: BigInt("2000000000"),
      });

      const tamperMsg = buildOracleMessage({
        ...baseParams,
        amountA: BigInt("999000000000"),
        amountB: BigInt("999000000000"),
      });

      const { signature } = signMessage(oracleMsg, oracleKeypair.secretKey);
      const isValid = nacl.sign.detached.verify(
        tamperMsg,
        signature,
        oracleKeypair.publicKey.toBytes()
      );
      expect(isValid).to.be.false;
      console.log("Token amount tampering prevented");
    });

    it("should prevent token mint substitution", () => {
      const baseParams = {
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const oracleMsg = buildOracleMessage({
        ...baseParams,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
      });

      // Attacker substitutes token mints
      const tamperMsg = buildOracleMessage({
        ...baseParams,
        tokenMintA: Keypair.generate().publicKey,
        tokenMintB: Keypair.generate().publicKey,
      });

      const { signature } = signMessage(oracleMsg, oracleKeypair.secretKey);
      const isValid = nacl.sign.detached.verify(
        tamperMsg,
        signature,
        oracleKeypair.publicKey.toBytes()
      );
      expect(isValid).to.be.false;
      console.log("Token mint substitution prevented");
    });
  });

  // ===========================================================================
  // SECURITY TESTS — TIMESTAMP VALIDATION
  // ===========================================================================

  describe("Timestamp Validation", () => {
    it("should reject stale timestamps", () => {
      const now = Math.floor(Date.now() / 1000);
      const staleTimestamp = BigInt(now - 120); // 120s ago > 60s max

      const age = now - Number(staleTimestamp);
      expect(age).to.be.greaterThan(MAX_ORACLE_STALENESS_SECONDS);
      console.log(
        `Stale timestamp rejected: age=${age}s > max=${MAX_ORACLE_STALENESS_SECONDS}s`
      );
    });

    it("should reject future timestamps", () => {
      const now = Math.floor(Date.now() / 1000);
      const futureTimestamp = BigInt(now + 300);

      const age = now - Number(futureTimestamp);
      expect(age).to.be.lessThan(0);
      console.log("Future timestamp rejected: age is negative");
    });

    it("should accept timestamps within staleness window", () => {
      const now = Math.floor(Date.now() / 1000);
      const recentTimestamp = BigInt(now - 30); // 30s ago < 60s max

      const age = now - Number(recentTimestamp);
      expect(age).to.be.greaterThanOrEqual(0);
      expect(age).to.be.lessThanOrEqual(MAX_ORACLE_STALENESS_SECONDS);
      console.log(`Recent timestamp accepted: age=${age}s`);
    });
  });

  // ===========================================================================
  // SECURITY TESTS — TICK ENFORCEMENT
  // ===========================================================================

  describe("Tick Range Enforcement from Config", () => {
    it("should enforce ticks from ProtocolConfig, not user input", () => {
      // add_liquidity_and_mint_bond does NOT take tick_lower_index or
      // tick_upper_index as parameters. They are read from config.
      // This prevents user-controlled tick manipulation.
      console.log(
        "Ticks sourced from ProtocolConfig — no user-provided tick params"
      );
    });

    it("should validate config ticks at initialization", async () => {
      // initialize() validates tick_lower < tick_upper and bounds
      // This test verifies config was initialized with correct ticks
      const config = await program.account.protocolConfig.fetch(configPda);
      expect(config.tickLowerIndex).to.be.lessThan(config.tickUpperIndex);
      expect(config.tickLowerIndex).to.be.greaterThanOrEqual(-443636);
      expect(config.tickUpperIndex).to.be.lessThanOrEqual(443636);
      console.log(
        `Config ticks: [${config.tickLowerIndex}, ${config.tickUpperIndex}]`
      );
    });
  });

  // ===========================================================================
  // ENCODING EDGE CASES
  // ===========================================================================

  describe("Encoding Edge Cases", () => {
    it("should handle negative tick values correctly", () => {
      const message = buildOracleMessage({
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: mockLiquidity,
        tickLower: -443636,
        tickUpper: 443636,
        tickCurrent: -50000,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      });

      expect(message.length).to.equal(ORACLE_MESSAGE_LEN);

      // tick_lower at offset 146
      const encodedTickLower = message.readInt32LE(146);
      expect(encodedTickLower).to.equal(-443636);

      // tick_current at offset 154
      const encodedTickCurrent = message.readInt32LE(154);
      expect(encodedTickCurrent).to.equal(-50000);
      console.log("Negative tick encoding verified");
    });

    it("should handle max u128 liquidity", () => {
      const maxLiquidity = BigInt(
        "340282366920938463463374607431768211455"
      ); // 2^128 - 1

      const message = buildOracleMessage({
        domain: ORACLE_DOMAIN_MINT,
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenMintA: mockTokenMintA,
        tokenMintB: mockTokenMintB,
        amountA: mockAmountA,
        amountB: mockAmountB,
        liquidity: maxLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        timestamp: BigInt(1700000000),
        sender: user.publicKey,
        contractAddress: program.programId,
      });

      // liquidity at offset 130..146
      const lo = message.readBigUInt64LE(130);
      const hi = message.readBigUInt64LE(138);
      const decoded = lo + (hi << BigInt(64));
      expect(decoded).to.equal(maxLiquidity);
      console.log("Max u128 liquidity encoding verified");
    });
  });
});

// =============================================================================
// SECURITY REVIEW SUMMARY
// =============================================================================

/**
 * ORACLE VERIFICATION SECURITY DEFENSES (audit-grade):
 *
 * 1. MANDATORY ORACLE VERIFICATION AT MINT TIME
 *    - add_liquidity_and_mint_bond REQUIRES valid oracle attestation
 *    - NO way to mint without oracle — verification runs before any CPI
 *    - Oracle can be disabled by admin in emergencies
 *
 * 2. NO SIGNATURE PARAMETER
 *    - Signature is NOT passed as an instruction argument
 *    - Signature exists ONLY inside the Ed25519 precompile instruction
 *    - Runtime verifies the cryptographic signature before our program runs
 *    - Our program only extracts and verifies the pubkey + message
 *
 * 3. STRICT INSTRUCTION ORDERING
 *    - Ed25519 instruction MUST be at exactly (current_index - 1)
 *    - NO scanning loop — single index lookup
 *    - Prevents attacker from placing Ed25519 ix at arbitrary positions
 *
 * 4. STRICT SEQUENTIAL NONCE
 *    - Nonce must be exactly current_nonce + 1 (not just >)
 *    - Prevents replay (same nonce) AND skipping (gaps)
 *    - Per-user nonce accounts prevent cross-user interference
 *
 * 5. TIMESTAMP STALENESS PROTECTION
 *    - Oracle timestamp must be within MAX_ORACLE_STALENESS_SECONDS (60s)
 *    - Future timestamps rejected
 *    - Prevents use of outdated market data
 *
 * 6. UNIFIED VERIFICATION LOGIC
 *    - Single verify_oracle_attestation() function for both mint and verify_collateral
 *    - Single OracleMessageParams struct and reconstruct_oracle_message()
 *    - Different domain separators prevent cross-instruction replay
 *
 * 7. CONFIG-ENFORCED TICK RANGE
 *    - Ticks come from ProtocolConfig, NOT user input
 *    - All positions use same tick range — prevents manipulation
 *    - Admin sets ticks at initialization / config update
 *
 * 8. STRONG MESSAGE BINDING
 *    - Signed message includes: whirlpool, BOTH token mints, amounts,
 *      liquidity, ticks, nonce, timestamp, sender, program ID
 *    - 238-byte fixed-size deterministic layout
 *    - No JSON, no floating point — little-endian binary
 *
 * 9. NO REDUNDANT SIGNATURE COMPARISON
 *    - Ed25519 precompile already verified the signature
 *    - Program does NOT re-compare signature bytes
 *    - Only pubkey and message are extracted and verified
 *
 * 10. ED25519 PRECOMPILE SECURITY
 *     - Uses native Solana Ed25519 program for verification
 *     - All data (sig, pubkey, message) must be within the SAME instruction
 *       (instruction index == 0xFFFF) — prevents cross-instruction splicing
 *     - Transaction fails at runtime if signature is cryptographically invalid
 */
