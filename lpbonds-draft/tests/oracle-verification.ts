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
// CONSTANTS
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

// Signature domain
const SIGNATURE_DOMAIN = "LP_BONDS_SOLANA_V1";
const CANONICAL_MESSAGE_LEN = 198;

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
 * Build canonical message for oracle signature.
 * MUST match exactly with Rust implementation in ed25519.rs
 */
function buildCanonicalMessage(params: {
  bondMint: PublicKey;
  positionMint: PublicKey;
  amount0: bigint;
  amount1: bigint;
  liquidity: bigint;
  tickLower: number;
  tickUpper: number;
  tickCurrent: number;
  nonce: bigint;
  sender: PublicKey;
  contractAddress: PublicKey;
}): Buffer {
  const message = Buffer.alloc(CANONICAL_MESSAGE_LEN);
  let offset = 0;

  // Domain separator (18 bytes)
  const domain = Buffer.from(SIGNATURE_DOMAIN, "utf-8");
  domain.copy(message, offset);
  offset += domain.length;

  // bond_mint (32 bytes)
  params.bondMint.toBuffer().copy(message, offset);
  offset += 32;

  // position_mint (32 bytes)
  params.positionMint.toBuffer().copy(message, offset);
  offset += 32;

  // amount0 (u64 LE, 8 bytes)
  message.writeBigUInt64LE(params.amount0, offset);
  offset += 8;

  // amount1 (u64 LE, 8 bytes)
  message.writeBigUInt64LE(params.amount1, offset);
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

  // Mock bond/position for testing
  let mockBondMint: Keypair;
  let mockPositionMint: Keypair;
  let positionCustodyPda: PublicKey;

  // Test data
  const mockLiquidity = BigInt("36583284382");
  const mockAmount0 = BigInt("1000000000");
  const mockAmount1 = BigInt("2000000000");
  const mockTickLower = -10000;
  const mockTickUpper = 10000;
  const mockTickCurrent = 500;

  before(async () => {
    // Generate keypairs
    admin = Keypair.generate();
    user = Keypair.generate();
    oracleKeypair = Keypair.generate();
    maliciousOracleKeypair = Keypair.generate();
    mockBondMint = Keypair.generate();
    mockPositionMint = Keypair.generate();

    // Airdrop SOL
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

    // Derive PDAs
    [configPda] = deriveConfigPda(program.programId);
    [oracleConfigPda] = deriveOracleConfigPda(program.programId);
    [noncePda] = deriveNoncePda(user.publicKey, program.programId);
    [positionCustodyPda] = derivePositionCustodyPda(
      mockBondMint.publicKey,
      program.programId
    );

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
    it("should initialize protocol config", async () => {
      const [bondAuthorityPda] = derivePda(
        [BOND_AUTHORITY_SEED],
        program.programId
      );

      const tx = await program.methods
        .initialize()
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

      const tx = await program.methods
        .updateOracleAuthority(newOracleKeypair.publicKey)
        .accounts({
          admin: admin.publicKey,
          oracleConfig: oracleConfigPda,
        })
        .signers([admin])
        .rpc();

      console.log("Oracle authority updated:", tx);

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
            admin: user.publicKey, // Not the admin
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

  describe("Canonical Message Building", () => {
    it("should build deterministic canonical message", () => {
      const params = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message1 = buildCanonicalMessage(params);
      const message2 = buildCanonicalMessage(params);

      expect(message1.equals(message2)).to.be.true;
      expect(message1.length).to.equal(CANONICAL_MESSAGE_LEN);

      console.log("Canonical message length:", message1.length);
      console.log("Message hex (first 64 bytes):", message1.slice(0, 64).toString("hex"));
    });

    it("should produce different messages for different nonces", () => {
      const baseParams = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message1 = buildCanonicalMessage({ ...baseParams, nonce: BigInt(1) });
      const message2 = buildCanonicalMessage({ ...baseParams, nonce: BigInt(2) });

      expect(message1.equals(message2)).to.be.false;
    });

    it("should produce different messages for different senders", () => {
      const baseParams = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        contractAddress: program.programId,
      };

      const message1 = buildCanonicalMessage({ ...baseParams, sender: user.publicKey });
      const message2 = buildCanonicalMessage({ ...baseParams, sender: admin.publicKey });

      expect(message1.equals(message2)).to.be.false;
    });
  });

  // ===========================================================================
  // ED25519 SIGNATURE TESTS
  // ===========================================================================

  describe("Ed25519 Signing", () => {
    it("should sign and verify message correctly", () => {
      const params = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message = buildCanonicalMessage(params);
      const { signature, publicKey } = signMessage(message, oracleKeypair.secretKey);

      // Verify signature locally
      const isValid = nacl.sign.detached.verify(
        message,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.true;
      expect(signature.length).to.equal(64);
      expect(publicKey.length).to.equal(32);

      console.log("Signature (base64):", signature.toString("base64"));
      console.log("Public key:", oracleKeypair.publicKey.toBase58());
    });

    it("should reject invalid signature", () => {
      const params = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message = buildCanonicalMessage(params);
      
      // Sign with different keypair
      const { signature } = signMessage(message, maliciousOracleKeypair.secretKey);

      // Verify with correct oracle pubkey should fail
      const isValid = nacl.sign.detached.verify(
        message,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.false;
    });

    it("should build Ed25519 instruction correctly", () => {
      const params = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message = buildCanonicalMessage(params);
      const { signature, publicKey } = signMessage(message, oracleKeypair.secretKey);

      const ed25519Ix = buildEd25519Instruction(publicKey, signature, message);

      expect(ed25519Ix.programId.toBase58()).to.equal(Ed25519Program.programId.toBase58());
      expect(ed25519Ix.data.length).to.be.greaterThan(0);

      console.log("Ed25519 instruction data length:", ed25519Ix.data.length);
    });
  });

  // ===========================================================================
  // SECURITY TESTS
  // ===========================================================================

  describe("Security Checks", () => {
    it("should prevent cross-user replay attack", () => {
      const victimParams = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey, // Victim
        contractAddress: program.programId,
      };

      const attackerParams = {
        ...victimParams,
        sender: admin.publicKey, // Attacker
      };

      const victimMessage = buildCanonicalMessage(victimParams);
      const attackerMessage = buildCanonicalMessage(attackerParams);

      // Messages should be different
      expect(victimMessage.equals(attackerMessage)).to.be.false;

      // Signature on victim message should not verify for attacker message
      const { signature } = signMessage(victimMessage, oracleKeypair.secretKey);

      const isValid = nacl.sign.detached.verify(
        attackerMessage,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.false;
      console.log("Cross-user replay attack prevented");
    });

    it("should prevent cross-position replay attack", () => {
      const otherBondMint = Keypair.generate();

      const originalParams = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const attackParams = {
        ...originalParams,
        bondMint: otherBondMint.publicKey, // Different position
      };

      const originalMessage = buildCanonicalMessage(originalParams);
      const attackMessage = buildCanonicalMessage(attackParams);

      expect(originalMessage.equals(attackMessage)).to.be.false;

      const { signature } = signMessage(originalMessage, oracleKeypair.secretKey);

      const isValid = nacl.sign.detached.verify(
        attackMessage,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.false;
      console.log("Cross-position replay attack prevented");
    });

    it("should bind signature to contract address", () => {
      const maliciousContract = Keypair.generate();

      const originalParams = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const attackParams = {
        ...originalParams,
        contractAddress: maliciousContract.publicKey, // Different contract
      };

      const originalMessage = buildCanonicalMessage(originalParams);
      const attackMessage = buildCanonicalMessage(attackParams);

      expect(originalMessage.equals(attackMessage)).to.be.false;

      const { signature } = signMessage(originalMessage, oracleKeypair.secretKey);

      const isValid = nacl.sign.detached.verify(
        attackMessage,
        signature,
        oracleKeypair.publicKey.toBytes()
      );

      expect(isValid).to.be.false;
      console.log("Contract address binding enforced");
    });

    it("should handle negative tick values correctly", () => {
      const params = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: mockLiquidity,
        tickLower: -443636, // MIN_TICK_INDEX
        tickUpper: 443636, // MAX_TICK_INDEX
        tickCurrent: -50000, // Negative current tick
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message = buildCanonicalMessage(params);
      expect(message.length).to.equal(CANONICAL_MESSAGE_LEN);

      // Verify tick values are encoded correctly
      const tickLowerOffset = 18 + 32 + 32 + 8 + 8 + 16;
      const encodedTickLower = message.readInt32LE(tickLowerOffset);
      expect(encodedTickLower).to.equal(-443636);

      const tickCurrentOffset = tickLowerOffset + 4 + 4;
      const encodedTickCurrent = message.readInt32LE(tickCurrentOffset);
      expect(encodedTickCurrent).to.equal(-50000);

      console.log("Negative tick encoding verified");
    });

    it("should handle large liquidity values (u128)", () => {
      const largeLiquidity = BigInt("340282366920938463463374607431768211455"); // Max u128

      const params = {
        bondMint: mockBondMint.publicKey,
        positionMint: mockPositionMint.publicKey,
        amount0: mockAmount0,
        amount1: mockAmount1,
        liquidity: largeLiquidity,
        tickLower: mockTickLower,
        tickUpper: mockTickUpper,
        tickCurrent: mockTickCurrent,
        nonce: BigInt(1),
        sender: user.publicKey,
        contractAddress: program.programId,
      };

      const message = buildCanonicalMessage(params);
      expect(message.length).to.equal(CANONICAL_MESSAGE_LEN);

      // Verify large liquidity is encoded correctly
      const liquidityOffset = 18 + 32 + 32 + 8 + 8;
      const liquidityLow = message.readBigUInt64LE(liquidityOffset);
      const liquidityHigh = message.readBigUInt64LE(liquidityOffset + 8);
      const decodedLiquidity = liquidityLow + (liquidityHigh << BigInt(64));

      expect(decodedLiquidity).to.equal(largeLiquidity);
      console.log("Large u128 liquidity encoding verified");
    });
  });
});

// =============================================================================
// SECURITY REVIEW SUMMARY
// =============================================================================

/**
 * ORACLE VERIFICATION SECURITY DEFENSES:
 *
 * 1. REPLAY ATTACK PREVENTION
 *    - Per-user nonce account with strictly increasing nonces
 *    - Nonce is part of signed message
 *    - Same signature cannot be reused
 *
 * 2. CROSS-USER REPLAY PREVENTION
 *    - Sender pubkey is part of signed message
 *    - Signature from one user cannot be used by another
 *
 * 3. CROSS-POSITION REPLAY PREVENTION
 *    - Bond mint and position mint are part of signed message
 *    - Signature for one position cannot be used for another
 *
 * 4. CONTRACT BINDING
 *    - Contract address (program ID) is part of signed message
 *    - Signature cannot be replayed on different contracts
 *
 * 5. ORACLE AUTHORITY VERIFICATION
 *    - Ed25519 signature verified against stored oracle authority
 *    - Only admin can update oracle authority
 *
 * 6. DETERMINISTIC SERIALIZATION
 *    - Fixed-size message format (198 bytes)
 *    - No JSON, no floating point
 *    - Little-endian byte order
 *    - Exact byte layout matches Rust implementation
 *
 * 7. ED25519 PRECOMPILE
 *    - Uses native Solana Ed25519 program for verification
 *    - Transaction fails if signature invalid
 *    - Verification happens before instruction executes
 */
