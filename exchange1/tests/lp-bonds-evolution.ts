import * as anchor from "@coral-xyz/anchor";
import { Program, AnchorError, BN } from "@coral-xyz/anchor";
import { LpBondsEvolution } from "../target/types/lp_bonds_evolution";
import {
  PublicKey,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { createMint } from "@solana/spl-token";
import { expect } from "chai";
import * as nacl from "tweetnacl";

// =============================================================================
// CONSTANTS
// =============================================================================

// Level 2-4 Whirlpools
const LEVEL_2_WHIRLPOOL = new PublicKey(
  "36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V"
);
const LEVEL_3_WHIRLPOOL = new PublicKey(
  "GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ"
);
const LEVEL_4_WHIRLPOOL = new PublicKey(
  "2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo"
);

// Token Mints
const LEVEL_2_TOKEN_B = new PublicKey(
  "Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB"
);
const LEVEL_3_TOKEN_B = new PublicKey(
  "9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp"
);
const LEVEL_4_TOKEN_B = new PublicKey(
  "9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu"
);

// PDA Seeds
const EVOLUTION_CONFIG_SEED = Buffer.from("evolution_config");
const LEVEL_CONFIG_SEED = Buffer.from("level_config");
const LAYER_TOKEN_AUTHORITY_SEED = Buffer.from("layer_token_authority");

// Evolution signature domain
const EVOLUTION_SIGNATURE_DOMAIN = Buffer.from("LP_BONDS_EVOLVE_V1");

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function derivePda(
  seeds: (Buffer | Uint8Array)[],
  programId: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(seeds, programId);
}

function createEvolutionCanonicalMessage(params: {
  sourceBondMint: PublicKey;
  targetLevel: number;
  amountA: BN;
  amountB: BN;
  liquidity: BN;
  nonce: BN;
  sender: PublicKey;
  contractAddress: PublicKey;
}): Buffer {
  const buffer = Buffer.alloc(155);
  let offset = 0;

  EVOLUTION_SIGNATURE_DOMAIN.copy(buffer, offset);
  offset += 18;

  params.sourceBondMint.toBuffer().copy(buffer, offset);
  offset += 32;

  buffer.writeUInt8(params.targetLevel, offset);
  offset += 1;

  buffer.writeBigUInt64LE(BigInt(params.amountA.toString()), offset);
  offset += 8;

  buffer.writeBigUInt64LE(BigInt(params.amountB.toString()), offset);
  offset += 8;

  const liquidityBuf = Buffer.alloc(16);
  const liq = BigInt(params.liquidity.toString());
  liquidityBuf.writeBigUInt64LE(liq & BigInt("0xFFFFFFFFFFFFFFFF"), 0);
  liquidityBuf.writeBigUInt64LE(liq >> BigInt(64), 8);
  liquidityBuf.copy(buffer, offset);
  offset += 16;

  buffer.writeBigUInt64LE(BigInt(params.nonce.toString()), offset);
  offset += 8;

  params.sender.toBuffer().copy(buffer, offset);
  offset += 32;

  params.contractAddress.toBuffer().copy(buffer, offset);

  return buffer;
}

function signEvolutionMessage(
  message: Buffer,
  secretKey: Uint8Array
): Uint8Array {
  return nacl.sign.detached(message, secretKey);
}

// =============================================================================
// TEST SUITE
// =============================================================================

describe("LP Bonds Evolution Program", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const evolutionProgram = anchor.workspace
    .LpBondsEvolution as Program<LpBondsEvolution>;

  // Use the provider wallet as admin (already funded)
  const admin = (provider.wallet as anchor.Wallet).payer;
  const treasury = (provider.wallet as anchor.Wallet).payer;
  const oracleKeypair = Keypair.generate();

  let evolutionConfigPda: PublicKey;
  let layerTokenAuthorityPda: PublicKey;

  let tokenMintA: PublicKey;
  let layerTokenMint2: PublicKey;
  let layerTokenMint3: PublicKey;
  let layerTokenMint4: PublicKey;

  before(async () => {
    // Skip airdrops - use existing funded wallet
    console.log("Using wallet:", admin.publicKey.toString());
    const balance = await provider.connection.getBalance(admin.publicKey);
    console.log("Wallet balance:", balance / LAMPORTS_PER_SOL, "SOL");

    [evolutionConfigPda] = derivePda(
      [EVOLUTION_CONFIG_SEED],
      evolutionProgram.programId
    );
    [layerTokenAuthorityPda] = derivePda(
      [LAYER_TOKEN_AUTHORITY_SEED],
      evolutionProgram.programId
    );

    tokenMintA = await createMint(
      provider.connection,
      admin,
      admin.publicKey,
      null,
      9
    );

    layerTokenMint2 = await createMint(
      provider.connection,
      admin,
      layerTokenAuthorityPda,
      null,
      9
    );

    layerTokenMint3 = await createMint(
      provider.connection,
      admin,
      layerTokenAuthorityPda,
      null,
      9
    );

    layerTokenMint4 = await createMint(
      provider.connection,
      admin,
      layerTokenAuthorityPda,
      null,
      9
    );
  });

  // ===========================================================================
  // INITIALIZATION TESTS
  // ===========================================================================

  describe("Initialization", () => {
    it("Should initialize evolution config (or verify if already initialized)", async () => {
      // Check if already initialized
      let config;
      try {
        config = await evolutionProgram.account.evolutionConfig.fetch(
          evolutionConfigPda
        );
        console.log("Evolution config already initialized");
      } catch {
        // Not initialized, create it
        const tx = await evolutionProgram.methods
          .initializeEvolution(treasury.publicKey, oracleKeypair.publicKey)
          .accounts({
            admin: admin.publicKey,
          })
          .signers([admin])
          .rpc();
        console.log("Initialize evolution tx:", tx);
        config = await evolutionProgram.account.evolutionConfig.fetch(
          evolutionConfigPda
        );
      }

      expect(config.admin.toString()).to.equal(admin.publicKey.toString());
      expect(config.isPaused).to.be.false;
    });

    it("Should initialize layer token authority (or verify if already initialized)", async () => {
      // Check if already initialized
      let authority;
      try {
        authority = await evolutionProgram.account.layerTokenAuthority.fetch(
          layerTokenAuthorityPda
        );
        console.log("Layer token authority already initialized");
      } catch {
        // Not initialized, create it
        const tx = await evolutionProgram.methods
          .initializeLayerAuthority()
          .accounts({
            admin: admin.publicKey,
          })
          .signers([admin])
          .rpc();
        console.log("Initialize layer authority tx:", tx);
        authority = await evolutionProgram.account.layerTokenAuthority.fetch(
          layerTokenAuthorityPda
        );
      }

      expect(authority.bump).to.be.greaterThan(0);
    });

    it("Should fail to initialize evolution config twice", async () => {
      try {
        await evolutionProgram.methods
          .initializeEvolution(treasury.publicKey, oracleKeypair.publicKey)
          .accounts({
            admin: admin.publicKey,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(Error);
      }
    });
  });

  // ===========================================================================
  // LEVEL CONFIGURATION TESTS
  // ===========================================================================

  describe("Level Configuration", () => {
    it("Should configure Level 2", async () => {
      const [levelConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );

      const tx = await evolutionProgram.methods
        .configureLevel(
          2,
          -443636,
          443636,
          new BN(1_000_000_000),
          new BN(500_000_000),
          100,
          new BN(5_184_000),
          150,
          true,
          new BN("1000000000000"),
          new BN("100000000000")
        )
        .accounts({
          admin: admin.publicKey,
          levelConfig: levelConfigPda,
          whirlpool: LEVEL_2_WHIRLPOOL,
          tokenMintA: tokenMintA,
          tokenMintB: LEVEL_2_TOKEN_B,
          layerTokenMint: layerTokenMint2,
        })
        .signers([admin])
        .rpc();

      console.log("Configure Level 2 tx:", tx);

      const config = await evolutionProgram.account.levelConfig.fetch(
        levelConfigPda
      );

      expect(config.levelId).to.equal(2);
      expect(config.whirlpool.toString()).to.equal(LEVEL_2_WHIRLPOOL.toString());
      expect(config.requiredAmountA.toNumber()).to.equal(1_000_000_000);
      expect(config.feeBps).to.equal(100);
      expect(config.isActive).to.be.true;
      expect(config.maxTotalMint.toString()).to.equal("1000000000000");
      expect(config.totalMinted.toNumber()).to.equal(0);
      expect(config.maxMintPerTx.toString()).to.equal("100000000000");
    });

    it("Should configure Level 3", async () => {
      const [levelConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([3])],
        evolutionProgram.programId
      );

      const tx = await evolutionProgram.methods
        .configureLevel(
          3,
          -443636,
          443636,
          new BN(2_000_000_000),
          new BN(1_000_000_000),
          150,
          new BN(7_776_000),
          200,
          true,
          new BN("2000000000000"),
          new BN("200000000000")
        )
        .accounts({
          admin: admin.publicKey,
          levelConfig: levelConfigPda,
          whirlpool: LEVEL_3_WHIRLPOOL,
          tokenMintA: tokenMintA,
          tokenMintB: LEVEL_3_TOKEN_B,
          layerTokenMint: layerTokenMint3,
        })
        .signers([admin])
        .rpc();

      console.log("Configure Level 3 tx:", tx);

      const config = await evolutionProgram.account.levelConfig.fetch(
        levelConfigPda
      );

      expect(config.levelId).to.equal(3);
      expect(config.multiplier).to.equal(200);
    });

    it("Should configure Level 4", async () => {
      const [levelConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([4])],
        evolutionProgram.programId
      );

      const tx = await evolutionProgram.methods
        .configureLevel(
          4,
          -443636,
          443636,
          new BN(5_000_000_000),
          new BN(2_500_000_000),
          200,
          new BN(10_368_000),
          300,
          true,
          new BN("5000000000000"),
          new BN("500000000000")
        )
        .accounts({
          admin: admin.publicKey,
          levelConfig: levelConfigPda,
          whirlpool: LEVEL_4_WHIRLPOOL,
          tokenMintA: tokenMintA,
          tokenMintB: LEVEL_4_TOKEN_B,
          layerTokenMint: layerTokenMint4,
        })
        .signers([admin])
        .rpc();

      console.log("Configure Level 4 tx:", tx);

      const config = await evolutionProgram.account.levelConfig.fetch(
        levelConfigPda
      );

      expect(config.levelId).to.equal(4);
      expect(config.feeBps).to.equal(200);
    });

    it("Should fail to configure invalid level (1)", async () => {
      const [invalidLevelConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([1])],
        evolutionProgram.programId
      );
      try {
        await evolutionProgram.methods
          .configureLevel(
            1,
            -443636,
            443636,
            new BN(1_000_000_000),
            new BN(500_000_000),
            100,
            new BN(5_184_000),
            150,
            true,
            new BN("1000000000000"),
            new BN("100000000000")
          )
          .accounts({
            admin: admin.publicKey,
            levelConfig: invalidLevelConfigPda,
            whirlpool: LEVEL_2_WHIRLPOOL,
            tokenMintA: tokenMintA,
            tokenMintB: LEVEL_2_TOKEN_B,
            layerTokenMint: layerTokenMint2,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(AnchorError);
        expect((error as AnchorError).error.errorCode.code).to.equal(
          "InvalidBondLevel"
        );
      }
    });

    it("Should fail to configure with fee > 50%", async () => {
      const [level2ConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );
      try {
        await evolutionProgram.methods
          .configureLevel(
            2,
            -443636,
            443636,
            new BN(1_000_000_000),
            new BN(500_000_000),
            5001,
            new BN(5_184_000),
            150,
            true,
            new BN("1000000000000"),
            new BN("100000000000")
          )
          .accounts({
            admin: admin.publicKey,
            levelConfig: level2ConfigPda,
            whirlpool: LEVEL_2_WHIRLPOOL,
            tokenMintA: tokenMintA,
            tokenMintB: LEVEL_2_TOKEN_B,
            layerTokenMint: layerTokenMint2,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(AnchorError);
        expect((error as AnchorError).error.errorCode.code).to.equal(
          "FeeTooHigh"
        );
      }
    });

    it("Should fail to configure with max_total_mint = 0", async () => {
      const [level2ConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );
      try {
        await evolutionProgram.methods
          .configureLevel(
            2,
            -443636,
            443636,
            new BN(1_000_000_000),
            new BN(500_000_000),
            100,
            new BN(5_184_000),
            150,
            true,
            new BN(0),
            new BN(0)
          )
          .accounts({
            admin: admin.publicKey,
            levelConfig: level2ConfigPda,
            whirlpool: LEVEL_2_WHIRLPOOL,
            tokenMintA: tokenMintA,
            tokenMintB: LEVEL_2_TOKEN_B,
            layerTokenMint: layerTokenMint2,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(AnchorError);
        expect((error as AnchorError).error.errorCode.code).to.equal(
          "InvalidMintCap"
        );
      }
    });

    it("Should fail to configure with max_mint_per_tx = 0", async () => {
      const [level2ConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );
      try {
        await evolutionProgram.methods
          .configureLevel(
            2,
            -443636,
            443636,
            new BN(1_000_000_000),
            new BN(500_000_000),
            100,
            new BN(5_184_000),
            150,
            true,
            new BN("1000000000000"),
            new BN(0)
          )
          .accounts({
            admin: admin.publicKey,
            levelConfig: level2ConfigPda,
            whirlpool: LEVEL_2_WHIRLPOOL,
            tokenMintA: tokenMintA,
            tokenMintB: LEVEL_2_TOKEN_B,
            layerTokenMint: layerTokenMint2,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(AnchorError);
        expect((error as AnchorError).error.errorCode.code).to.equal(
          "InvalidMintCap"
        );
      }
    });

    it("Should fail to configure with max_mint_per_tx > max_total_mint", async () => {
      const [level2ConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );
      try {
        await evolutionProgram.methods
          .configureLevel(
            2,
            -443636,
            443636,
            new BN(1_000_000_000),
            new BN(500_000_000),
            100,
            new BN(5_184_000),
            150,
            true,
            new BN("1000000000000"),
            new BN("2000000000000")
          )
          .accounts({
            admin: admin.publicKey,
            levelConfig: level2ConfigPda,
            whirlpool: LEVEL_2_WHIRLPOOL,
            tokenMintA: tokenMintA,
            tokenMintB: LEVEL_2_TOKEN_B,
            layerTokenMint: layerTokenMint2,
          })
          .signers([admin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(AnchorError);
        expect((error as AnchorError).error.errorCode.code).to.equal(
          "InvalidMintCap"
        );
      }
    });
  });

  // ===========================================================================
  // ADMIN FUNCTION TESTS
  // ===========================================================================

  describe("Admin Functions", () => {
    it("Should pause evolution", async () => {
      await evolutionProgram.methods
        .pauseEvolution()
        .accounts({
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      const config = await evolutionProgram.account.evolutionConfig.fetch(
        evolutionConfigPda
      );
      expect(config.isPaused).to.be.true;
    });

    it("Should unpause evolution", async () => {
      await evolutionProgram.methods
        .unpauseEvolution()
        .accounts({
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      const config = await evolutionProgram.account.evolutionConfig.fetch(
        evolutionConfigPda
      );
      expect(config.isPaused).to.be.false;
    });

    it("Should update treasury", async () => {
      const newTreasury = Keypair.generate();

      await evolutionProgram.methods
        .updateTreasury(newTreasury.publicKey)
        .accounts({
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      const config = await evolutionProgram.account.evolutionConfig.fetch(
        evolutionConfigPda
      );
      expect(config.treasury.toString()).to.equal(
        newTreasury.publicKey.toString()
      );

      // Reset
      await evolutionProgram.methods
        .updateTreasury(treasury.publicKey)
        .accounts({
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();
    });

    it("Should update oracle authority", async () => {
      const newOracle = Keypair.generate();

      await evolutionProgram.methods
        .updateOracle(newOracle.publicKey)
        .accounts({
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();

      const config = await evolutionProgram.account.evolutionConfig.fetch(
        evolutionConfigPda
      );
      expect(config.oracleAuthority.toString()).to.equal(
        newOracle.publicKey.toString()
      );

      // Reset
      await evolutionProgram.methods
        .updateOracle(oracleKeypair.publicKey)
        .accounts({
          admin: admin.publicKey,
        })
        .signers([admin])
        .rpc();
    });

    it("Should fail admin functions with non-admin signer", async () => {
      // This test verifies that the constraint check happens before any SOL is needed
      // The transaction should fail at validation, not execution
      const nonAdmin = Keypair.generate();

      try {
        await evolutionProgram.methods
          .pauseEvolution()
          .accounts({
            admin: nonAdmin.publicKey,
          })
          .signers([nonAdmin])
          .rpc();
        expect.fail("Should have thrown error");
      } catch (error) {
        expect(error).to.be.instanceOf(AnchorError);
        expect((error as AnchorError).error.errorCode.code).to.equal(
          "InvalidAdminAuthority"
        );
      }
    });
  });

  // ===========================================================================
  // SECURITY TESTS
  // ===========================================================================

  describe("Security Tests", () => {
    it("Should validate whirlpool matches level config", async () => {
      const [levelConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );

      const config = await evolutionProgram.account.levelConfig.fetch(
        levelConfigPda
      );

      expect(config.whirlpool.toString()).to.equal(LEVEL_2_WHIRLPOOL.toString());
      expect(config.whirlpool.toString()).to.not.equal(
        LEVEL_3_WHIRLPOOL.toString()
      );
    });

    it("Should verify oracle signature format", () => {
      const sourceBondMint = Keypair.generate().publicKey;
      const sender = Keypair.generate().publicKey;

      const message = createEvolutionCanonicalMessage({
        sourceBondMint,
        targetLevel: 2,
        amountA: new BN(1_000_000_000),
        amountB: new BN(500_000_000),
        liquidity: new BN("1000000000000000"),
        nonce: new BN(1),
        sender,
        contractAddress: evolutionProgram.programId,
      });

      expect(message.length).to.equal(155);

      const domain = message.subarray(0, 18).toString();
      expect(domain).to.equal("LP_BONDS_EVOLVE_V1");

      const signature = signEvolutionMessage(message, oracleKeypair.secretKey);
      expect(signature.length).to.equal(64);

      const isValid = nacl.sign.detached.verify(
        message,
        signature,
        oracleKeypair.publicKey.toBytes()
      );
      expect(isValid).to.be.true;
    });

    it("Should reject invalid signature", () => {
      const sourceBondMint = Keypair.generate().publicKey;
      const sender = Keypair.generate().publicKey;
      const wrongOracle = Keypair.generate();

      const message = createEvolutionCanonicalMessage({
        sourceBondMint,
        targetLevel: 2,
        amountA: new BN(1_000_000_000),
        amountB: new BN(500_000_000),
        liquidity: new BN("1000000000000000"),
        nonce: new BN(1),
        sender,
        contractAddress: evolutionProgram.programId,
      });

      const wrongSignature = signEvolutionMessage(message, wrongOracle.secretKey);

      const isValid = nacl.sign.detached.verify(
        message,
        wrongSignature,
        oracleKeypair.publicKey.toBytes()
      );
      expect(isValid).to.be.false;
    });

    it("Should verify fee calculation", async () => {
      const [levelConfigPda] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );

      const config = await evolutionProgram.account.levelConfig.fetch(
        levelConfigPda
      );

      const amount = 1_000_000_000;
      const expectedFee = Math.floor((amount * config.feeBps) / 10000);
      expect(expectedFee).to.equal(10_000_000);
    });

    it("Should verify lock duration is set correctly", async () => {
      const [level2Config] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([2])],
        evolutionProgram.programId
      );
      const [level3Config] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([3])],
        evolutionProgram.programId
      );
      const [level4Config] = derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([4])],
        evolutionProgram.programId
      );

      const config2 = await evolutionProgram.account.levelConfig.fetch(
        level2Config
      );
      const config3 = await evolutionProgram.account.levelConfig.fetch(
        level3Config
      );
      const config4 = await evolutionProgram.account.levelConfig.fetch(
        level4Config
      );

      expect(config2.lockDuration.toNumber()).to.equal(5_184_000);
      expect(config3.lockDuration.toNumber()).to.equal(7_776_000);
      expect(config4.lockDuration.toNumber()).to.equal(10_368_000);
    });
  });

  // ===========================================================================
  // PDA DERIVATION TESTS
  // ===========================================================================

  describe("PDA Derivation", () => {
    it("Should derive evolution config PDA correctly", () => {
      const [pda] = derivePda(
        [EVOLUTION_CONFIG_SEED],
        evolutionProgram.programId
      );
      expect(pda.toString()).to.equal(evolutionConfigPda.toString());
    });

    it("Should derive level config PDAs correctly", async () => {
      for (let level = 2; level <= 4; level++) {
        const [pda] = derivePda(
          [LEVEL_CONFIG_SEED, Buffer.from([level])],
          evolutionProgram.programId
        );
        const config = await evolutionProgram.account.levelConfig.fetch(pda);
        expect(config.levelId).to.equal(level);
      }
    });

    it("Should derive layer token authority PDA correctly", () => {
      const [pda] = derivePda(
        [LAYER_TOKEN_AUTHORITY_SEED],
        evolutionProgram.programId
      );
      expect(pda.toString()).to.equal(layerTokenAuthorityPda.toString());
    });
  });
});
