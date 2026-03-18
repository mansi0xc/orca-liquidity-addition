/**
 * Admin Configuration Script for LP Bonds Protocol (v2 - Optimized & Hardened)
 *
 * This script initializes and configures the entire protocol after fresh deployment.
 * Uses ~/.config/solana/id.json as the admin wallet.
 *
 * Safe to rerun -- checks account existence before initializing.
 *
 * Usage:
 *   npx ts-node scripts/admin-configure.ts
 *
 * Environment:
 *   ANCHOR_WALLET  - wallet path (defaults to ~/.config/solana/id.json)
 *   ANCHOR_PROVIDER_URL - RPC URL (defaults to https://api.devnet.solana.com)
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  ComputeBudgetProgram,
  Connection,
  SystemProgram,
} from "@solana/web3.js";
import * as fs from "fs";
import * as os from "os";

// =============================================================================
// PROTOCOL ADDRESSES (Whirlpools & Tokens - from existing devnet deployment)
// =============================================================================

const WHIRLPOOL_PROGRAM_ID = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");

const LEVEL_1_WHIRLPOOL = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");
const LEVEL_1_TOKEN_A = new PublicKey("So11111111111111111111111111111111111111112");
const LEVEL_1_TOKEN_B = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

const LEVEL_2_WHIRLPOOL = new PublicKey("36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V");
const LEVEL_2_TOKEN_A = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const LEVEL_2_TOKEN_B = new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB");

const LEVEL_3_WHIRLPOOL = new PublicKey("GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ");
const LEVEL_3_TOKEN_A = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const LEVEL_3_TOKEN_B = new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp");

const LEVEL_4_WHIRLPOOL = new PublicKey("2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo");
const LEVEL_4_TOKEN_A = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const LEVEL_4_TOKEN_B = new PublicKey("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu");

// =============================================================================
// LEVEL PARAMETERS
// =============================================================================

interface LevelParams {
  level: number;
  whirlpool: PublicKey;
  tokenMintA: PublicKey;
  tokenMintB: PublicKey;
  tickLower: number;
  tickUpper: number;
  requiredAmountA: BN;
  requiredAmountB: BN;
  feeBps: number;
  lockDuration: BN;
  multiplier: number;
}

const LEVEL_CONFIGS: LevelParams[] = [
  {
    level: 2,
    whirlpool: LEVEL_2_WHIRLPOOL,
    tokenMintA: LEVEL_2_TOKEN_A,
    tokenMintB: LEVEL_2_TOKEN_B,
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(2_000_000_000),
    requiredAmountB: new BN(1_000_000_000),
    feeBps: 100,
    lockDuration: new BN(5_184_000),
    multiplier: 150,
  },
  {
    level: 3,
    whirlpool: LEVEL_3_WHIRLPOOL,
    tokenMintA: LEVEL_3_TOKEN_A,
    tokenMintB: LEVEL_3_TOKEN_B,
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(3_000_000_000),
    requiredAmountB: new BN(1_500_000_000),
    feeBps: 150,
    lockDuration: new BN(7_776_000),
    multiplier: 200,
  },
  {
    level: 4,
    whirlpool: LEVEL_4_WHIRLPOOL,
    tokenMintA: LEVEL_4_TOKEN_A,
    tokenMintB: LEVEL_4_TOKEN_B,
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(4_000_000_000),
    requiredAmountB: new BN(2_000_000_000),
    feeBps: 200,
    lockDuration: new BN(10_368_000),
    multiplier: 300,
  },
];

const LEVEL_1_LOCK_DURATION = new BN(2_592_000); // 30 days

// PDA seeds
const CONFIG_SEED = Buffer.from("config");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const ORACLE_CONFIG_SEED = Buffer.from("oracle_config");
const EVOLUTION_CONFIG_SEED = Buffer.from("evolution_config");
const LAYER_TOKEN_AUTHORITY_SEED = Buffer.from("layer_token_authority");
const LEVEL_CONFIG_SEED = Buffer.from("level_config");

// =============================================================================
// HELPERS
// =============================================================================

function computeBudgetIx(units: number = 400_000, microLamports: number = 5_000) {
  return [
    ComputeBudgetProgram.setComputeUnitLimit({ units }),
    ComputeBudgetProgram.setComputeUnitPrice({ microLamports }),
  ];
}

function explorerLink(sig: string): string {
  return `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
}

async function accountExists(connection: Connection, address: PublicKey): Promise<boolean> {
  const info = await connection.getAccountInfo(address);
  return info !== null;
}

async function confirmTx(connection: Connection, sig: string): Promise<void> {
  await connection.confirmTransaction(sig, "confirmed");
}

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  console.log("=".repeat(70));
  console.log("LP BONDS PROTOCOL - ADMIN CONFIGURATION");
  console.log("=".repeat(70));

  // --- Setup provider ---
  const walletPath = process.env.ANCHOR_WALLET || "~/.config/solana/id.json";
  const resolvedPath = walletPath.replace("~", os.homedir());
  const keypairData = JSON.parse(fs.readFileSync(resolvedPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const rpcUrl = process.env.ANCHOR_PROVIDER_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  console.log("\nAdmin wallet:  ", admin.publicKey.toBase58());
  console.log("RPC URL:       ", rpcUrl);

  const balance = await connection.getBalance(admin.publicKey);
  console.log("Admin balance: ", balance / 1e9, "SOL\n");

  if (balance < 0.5 * 1e9) {
    throw new Error("Admin wallet needs at least 0.5 SOL for deployment and configuration");
  }

  // --- Load IDLs ---
  const lpBondsIdl = JSON.parse(fs.readFileSync("./target/idl/lp_bonds.json", "utf-8"));
  const evolutionIdl = JSON.parse(fs.readFileSync("./target/idl/lp_bonds_evolution.json", "utf-8"));

  const lpBondsProgram = new Program(lpBondsIdl, provider);
  const evolutionProgram = new Program(evolutionIdl, provider);

  const LP_BONDS_PROGRAM_ID = lpBondsProgram.programId;
  const EVOLUTION_PROGRAM_ID = evolutionProgram.programId;

  console.log("LP Bonds Program ID:  ", LP_BONDS_PROGRAM_ID.toBase58());
  console.log("Evolution Program ID: ", EVOLUTION_PROGRAM_ID.toBase58());

  // --- Derive PDAs ---
  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], LP_BONDS_PROGRAM_ID);
  const [bondAuthorityPda] = PublicKey.findProgramAddressSync([BOND_AUTHORITY_SEED], LP_BONDS_PROGRAM_ID);
  const [oracleConfigPda] = PublicKey.findProgramAddressSync([ORACLE_CONFIG_SEED], LP_BONDS_PROGRAM_ID);
  const [evolutionConfigPda] = PublicKey.findProgramAddressSync([EVOLUTION_CONFIG_SEED], EVOLUTION_PROGRAM_ID);
  const [layerTokenAuthorityPda] = PublicKey.findProgramAddressSync([LAYER_TOKEN_AUTHORITY_SEED], EVOLUTION_PROGRAM_ID);

  console.log("\n--- PDAs ---");
  console.log("Config PDA:              ", configPda.toBase58());
  console.log("Bond Authority PDA:      ", bondAuthorityPda.toBase58());
  console.log("Oracle Config PDA:       ", oracleConfigPda.toBase58());
  console.log("Evolution Config PDA:    ", evolutionConfigPda.toBase58());
  console.log("Layer Token Authority PDA:", layerTokenAuthorityPda.toBase58());

  const txSignatures: { step: string; signature: string }[] = [];

  // =========================================================================
  // STEP 1: Initialize LP Bonds Base Protocol
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 1: Initialize LP Bonds Base Protocol");
  console.log("-".repeat(70));

  if (await accountExists(connection, configPda)) {
    console.log("  Already initialized. Skipping.");
  } else {
    const sig = await lpBondsProgram.methods
      .initialize(
        LEVEL_1_WHIRLPOOL,
        LEVEL_1_TOKEN_A,
        LEVEL_1_TOKEN_B,
        LEVEL_1_LOCK_DURATION,
      )
      .accountsPartial({
        admin: admin.publicKey,
        config: configPda,
        bondAuthority: bondAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await confirmTx(connection, sig);
    txSignatures.push({ step: "Initialize LP Bonds", signature: sig });
    console.log("  Done:", explorerLink(sig));
  }

  // =========================================================================
  // STEP 2: Initialize Oracle
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 2: Initialize Oracle");
  console.log("-".repeat(70));

  if (await accountExists(connection, oracleConfigPda)) {
    console.log("  Already initialized. Skipping.");
  } else {
    const sig = await lpBondsProgram.methods
      .initializeOracle(admin.publicKey)
      .accountsPartial({
        admin: admin.publicKey,
        config: configPda,
        oracleConfig: oracleConfigPda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await confirmTx(connection, sig);
    txSignatures.push({ step: "Initialize Oracle", signature: sig });
    console.log("  Done:", explorerLink(sig));
  }

  // =========================================================================
  // STEP 3: Initialize Evolution Config
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 3: Initialize Evolution Config");
  console.log("-".repeat(70));

  if (await accountExists(connection, evolutionConfigPda)) {
    console.log("  Already initialized. Skipping.");
  } else {
    const sig = await evolutionProgram.methods
      .initializeEvolution(
        admin.publicKey,       // treasury (admin acts as treasury for now)
        admin.publicKey,       // oracle authority (admin signs for now)
        LP_BONDS_PROGRAM_ID,   // configurable base program ID
      )
      .accountsPartial({
        admin: admin.publicKey,
        evolutionConfig: evolutionConfigPda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await confirmTx(connection, sig);
    txSignatures.push({ step: "Initialize Evolution", signature: sig });
    console.log("  Done:", explorerLink(sig));
  }

  // =========================================================================
  // STEP 4: Initialize Layer Token Authority
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 4: Initialize Layer Token Authority");
  console.log("-".repeat(70));

  if (await accountExists(connection, layerTokenAuthorityPda)) {
    console.log("  Already initialized. Skipping.");
  } else {
    const sig = await evolutionProgram.methods
      .initializeLayerAuthority()
      .accountsPartial({
        admin: admin.publicKey,
        evolutionConfig: evolutionConfigPda,
        layerTokenAuthority: layerTokenAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await confirmTx(connection, sig);
    txSignatures.push({ step: "Initialize Layer Token Authority", signature: sig });
    console.log("  Done:", explorerLink(sig));
  }

  // =========================================================================
  // STEP 5: Layer Token Mints
  // =========================================================================
  // The layer_token_mint for each level is the whirlpool's token_b.
  // Per EVM parity, the protocol mints these tokens internally during evolution
  // (curToken1.mint(address(this), ...)), NOT transferred from the user.
  //
  // IMPORTANT: After running this script, run transfer-mint-authority.ts to
  // transfer mint authority of these tokens to the layer_token_authority PDA.

  console.log("\n" + "-".repeat(70));
  console.log("STEP 5: Layer Token Mints (= Whirlpool Token B)");
  console.log("-".repeat(70));

  const layerTokenMints: Map<number, PublicKey> = new Map();
  for (const cfg of LEVEL_CONFIGS) {
    layerTokenMints.set(cfg.level, cfg.tokenMintB);
    console.log(`  Level ${cfg.level} layer token = token_b: ${cfg.tokenMintB.toBase58()}`);
  }

  // =========================================================================
  // STEP 6: Configure Levels 2-4
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 6: Configure Evolution Levels");
  console.log("-".repeat(70));

  for (const cfg of LEVEL_CONFIGS) {
    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([cfg.level])],
      EVOLUTION_PROGRAM_ID,
    );

    const layerMint = layerTokenMints.get(cfg.level)!;

    console.log(`\n  Level ${cfg.level}:`);
    console.log(`    PDA:         ${levelConfigPda.toBase58()}`);
    console.log(`    Whirlpool:   ${cfg.whirlpool.toBase58()}`);
    console.log(`    Token A:     ${cfg.tokenMintA.toBase58()}`);
    console.log(`    Token B:     ${cfg.tokenMintB.toBase58()}`);
    console.log(`    Layer Mint:  ${layerMint.toBase58()} (= token_b)`);
    console.log(`    Fee BPS:     ${cfg.feeBps}`);
    console.log(`    Lock:        ${cfg.lockDuration.toNumber()} seconds`);
    console.log(`    Multiplier:  ${cfg.multiplier}`);

    const sig = await evolutionProgram.methods
      .configureLevel(
        cfg.level,
        cfg.tickLower,
        cfg.tickUpper,
        cfg.requiredAmountA,
        cfg.requiredAmountB,
        cfg.feeBps,
        cfg.lockDuration,
        cfg.multiplier,
        true, // is_active
      )
      .accountsPartial({
        admin: admin.publicKey,
        evolutionConfig: evolutionConfigPda,
        levelConfig: levelConfigPda,
        whirlpool: cfg.whirlpool,
        tokenMintA: cfg.tokenMintA,
        tokenMintB: cfg.tokenMintB,
        layerTokenMint: layerMint, // = token_b (must match whirlpool's token pair)
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await confirmTx(connection, sig);
    txSignatures.push({ step: `Configure Level ${cfg.level}`, signature: sig });
    console.log(`    Done:`, explorerLink(sig));
  }

  // =========================================================================
  // STEP 7: Verify All Configuration
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 7: Verification");
  console.log("-".repeat(70));

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const lpAccounts = lpBondsProgram.account as any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const evoAccounts = evolutionProgram.account as any;

  const configData = await lpAccounts.protocolConfig.fetch(configPda);
  console.log("\n  LP Bonds ProtocolConfig:");
  console.log("    admin:               ", configData.admin.toBase58());
  console.log("    allowlistedWhirlpool:", configData.allowlistedWhirlpool.toBase58());
  console.log("    tokenMintA:          ", configData.tokenMintA.toBase58());
  console.log("    tokenMintB:          ", configData.tokenMintB.toBase58());
  console.log("    lockDuration:        ", configData.lockDuration.toString());
  console.log("    bondCounter:         ", configData.bondCounter.toString());
  console.log("    isPaused:            ", configData.isPaused);

  const evoData = await evoAccounts.evolutionConfig.fetch(evolutionConfigPda);
  console.log("\n  Evolution Config:");
  console.log("    admin:               ", evoData.admin.toBase58());
  console.log("    treasury:            ", evoData.treasury.toBase58());
  console.log("    oracleAuthority:     ", evoData.oracleAuthority.toBase58());
  console.log("    lpBondsProgramId:    ", evoData.lpBondsProgramId.toBase58());
  console.log("    isPaused:            ", evoData.isPaused);
  console.log("    evolutionCounter:    ", evoData.evolutionCounter.toString());

  for (const cfg of LEVEL_CONFIGS) {
    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([cfg.level])],
      EVOLUTION_PROGRAM_ID,
    );
    const levelData = await evoAccounts.levelConfig.fetch(levelConfigPda);
    console.log(`\n  Level ${cfg.level} Config:`);
    console.log("    whirlpool:   ", levelData.whirlpool.toBase58());
    console.log("    tokenMintA:  ", levelData.tokenMintA.toBase58());
    console.log("    tokenMintB:  ", levelData.tokenMintB.toBase58());
    console.log("    layerToken:  ", levelData.layerTokenMint.toBase58());
    console.log("    reqAmountA:  ", levelData.requiredAmountA.toString());
    console.log("    reqAmountB:  ", levelData.requiredAmountB.toString());
    console.log("    feeBps:      ", levelData.feeBps);
    console.log("    lockDuration:", levelData.lockDuration.toString());
    console.log("    multiplier:  ", levelData.multiplier);
    console.log("    isActive:    ", levelData.isActive);
  }

  // =========================================================================
  // SUMMARY
  // =========================================================================

  console.log("\n" + "=".repeat(70));
  console.log("CONFIGURATION COMPLETE");
  console.log("=".repeat(70));

  console.log("\nProgram IDs:");
  console.log("  LP Bonds:   ", LP_BONDS_PROGRAM_ID.toBase58());
  console.log("  Evolution:  ", EVOLUTION_PROGRAM_ID.toBase58());

  console.log("\nAdmin:        ", admin.publicKey.toBase58());

  console.log("\nPDAs:");
  console.log("  Config:            ", configPda.toBase58());
  console.log("  Bond Authority:    ", bondAuthorityPda.toBase58());
  console.log("  Oracle Config:     ", oracleConfigPda.toBase58());
  console.log("  Evolution Config:  ", evolutionConfigPda.toBase58());
  console.log("  Layer Token Auth:  ", layerTokenAuthorityPda.toBase58());

  console.log("\nLayer Token Mints (= Whirlpool Token B):");
  for (const [level, mint] of layerTokenMints) {
    console.log(`  Level ${level}: ${mint.toBase58()}`);
  }

  console.log("\nTransaction Log:");
  for (const { step, signature } of txSignatures) {
    console.log(`  ${step}: ${signature}`);
  }

  console.log("\nAll", txSignatures.length, "transactions confirmed.");
  console.log("=".repeat(70));
}

main().catch((err) => {
  console.error("\nFATAL ERROR:", err.message || err);
  if (err.logs) {
    console.error("\nProgram Logs:");
    err.logs.forEach((log: string) => console.error("  ", log));
  }
  process.exit(1);
});
