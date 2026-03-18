/**
 * Configure LP Bonds Protocol
 *
 * Comprehensive configuration script for the LP Bonds and Evolution programs.
 * Initializes all PDAs and configures evolution levels 2-4.
 *
 * IMPORTANT: The layer_token_mint for each level MUST be the whirlpool's token_b.
 * The protocol transfers user's token_b during evolution (not mints).
 *
 * Safe to rerun -- checks account existence before initializing.
 *
 * Usage:
 *   npx ts-node --project tsconfig.scripts.json scripts/configure-bonds.ts
 *
 * Environment:
 *   ANCHOR_WALLET       - wallet path (defaults to ~/.config/solana/id.json)
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
// PROTOCOL ADDRESSES
// =============================================================================

// Level 1 (Base LP Bonds - wSOL / LAYER token pair)
const LEVEL_1_WHIRLPOOL = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");
const LEVEL_1_TOKEN_A = new PublicKey("So11111111111111111111111111111111111111112"); // wSOL
const LEVEL_1_TOKEN_B = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const LEVEL_1_LOCK_DURATION = new BN(2_592_000); // 30 days in seconds

// Level 2-4 Evolution Whirlpools
// CRITICAL: tokenMintB and layerTokenMint MUST match the whirlpool's actual token_b
const LEVEL_CONFIGS = [
  {
    level: 2,
    whirlpool: new PublicKey("36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V"),
    tokenMintA: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
    tokenMintB: new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB"),
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(2_000_000_000), // 2B
    requiredAmountB: new BN(1_000_000_000), // 1B
    feeBps: 100,                            // 1%
    lockDuration: new BN(5_184_000),        // 60 days
    multiplier: 150,                        // 1.5x
  },
  {
    level: 3,
    whirlpool: new PublicKey("GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ"),
    tokenMintA: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
    tokenMintB: new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp"),
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(3_000_000_000), // 3B
    requiredAmountB: new BN(1_500_000_000), // 1.5B
    feeBps: 150,                            // 1.5%
    lockDuration: new BN(7_776_000),        // 90 days
    multiplier: 200,                        // 2x
  },
  {
    level: 4,
    whirlpool: new PublicKey("2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo"),
    tokenMintA: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
    tokenMintB: new PublicKey("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu"),
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(4_000_000_000), // 4B
    requiredAmountB: new BN(2_000_000_000), // 2B
    feeBps: 200,                            // 2%
    lockDuration: new BN(10_368_000),       // 120 days
    multiplier: 300,                        // 3x
  },
];

// =============================================================================
// PDA SEEDS
// =============================================================================

const CONFIG_SEED = Buffer.from("config");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const ORACLE_CONFIG_SEED = Buffer.from("oracle_config");
const EVOLUTION_CONFIG_SEED = Buffer.from("evolution_config");
const LAYER_TOKEN_AUTHORITY_SEED = Buffer.from("layer_token_authority");
const LEVEL_CONFIG_SEED = Buffer.from("level_config");

// =============================================================================
// HELPERS
// =============================================================================

function computeBudgetIx(units = 400_000, microLamports = 5_000) {
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

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  console.log("=".repeat(70));
  console.log("LP BONDS PROTOCOL - CONFIGURATION");
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
  console.log("Config PDA:               ", configPda.toBase58());
  console.log("Bond Authority PDA:       ", bondAuthorityPda.toBase58());
  console.log("Oracle Config PDA:        ", oracleConfigPda.toBase58());
  console.log("Evolution Config PDA:     ", evolutionConfigPda.toBase58());
  console.log("Layer Token Authority PDA:", layerTokenAuthorityPda.toBase58());

  const txSignatures: { step: string; signature: string }[] = [];

  // =========================================================================
  // STEP 1: Initialize LP Bonds Base Protocol
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 1: Initialize LP Bonds Base Protocol");
  console.log("-".repeat(70));

  if (await accountExists(connection, configPda)) {
    console.log("  [SKIP] Already initialized.");
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

    await connection.confirmTransaction(sig, "confirmed");
    txSignatures.push({ step: "Initialize LP Bonds", signature: sig });
    console.log("  [OK]", explorerLink(sig));
  }

  // =========================================================================
  // STEP 2: Initialize Oracle
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 2: Initialize Oracle");
  console.log("-".repeat(70));

  if (await accountExists(connection, oracleConfigPda)) {
    console.log("  [SKIP] Already initialized.");
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

    await connection.confirmTransaction(sig, "confirmed");
    txSignatures.push({ step: "Initialize Oracle", signature: sig });
    console.log("  [OK]", explorerLink(sig));
  }

  // =========================================================================
  // STEP 3: Initialize Evolution Config
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 3: Initialize Evolution Config");
  console.log("-".repeat(70));

  if (await accountExists(connection, evolutionConfigPda)) {
    console.log("  [SKIP] Already initialized.");
  } else {
    const sig = await evolutionProgram.methods
      .initializeEvolution(
        admin.publicKey,       // treasury
        admin.publicKey,       // oracle authority (admin signs for devnet)
        LP_BONDS_PROGRAM_ID,   // base LP Bonds program ID
      )
      .accountsPartial({
        admin: admin.publicKey,
        evolutionConfig: evolutionConfigPda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await connection.confirmTransaction(sig, "confirmed");
    txSignatures.push({ step: "Initialize Evolution", signature: sig });
    console.log("  [OK]", explorerLink(sig));
  }

  // =========================================================================
  // STEP 4: Initialize Layer Token Authority
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 4: Initialize Layer Token Authority");
  console.log("-".repeat(70));

  if (await accountExists(connection, layerTokenAuthorityPda)) {
    console.log("  [SKIP] Already initialized.");
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

    await connection.confirmTransaction(sig, "confirmed");
    txSignatures.push({ step: "Initialize Layer Token Authority", signature: sig });
    console.log("  [OK]", explorerLink(sig));
  }

  // =========================================================================
  // STEP 5: Configure Evolution Levels 2-4
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 5: Configure Evolution Levels");
  console.log("-".repeat(70));

  for (const cfg of LEVEL_CONFIGS) {
    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([cfg.level])],
      EVOLUTION_PROGRAM_ID,
    );

    console.log(`\n  Level ${cfg.level}:`);
    console.log(`    PDA:          ${levelConfigPda.toBase58()}`);
    console.log(`    Whirlpool:    ${cfg.whirlpool.toBase58()}`);
    console.log(`    Token A:      ${cfg.tokenMintA.toBase58()}`);
    console.log(`    Token B:      ${cfg.tokenMintB.toBase58()}`);
    console.log(`    Layer Mint:   ${cfg.tokenMintB.toBase58()} (= token_b)`);
    console.log(`    Required A:   ${cfg.requiredAmountA.toString()}`);
    console.log(`    Required B:   ${cfg.requiredAmountB.toString()}`);
    console.log(`    Fee BPS:      ${cfg.feeBps} (${cfg.feeBps / 100}%)`);
    console.log(`    Lock:         ${cfg.lockDuration.toNumber()} seconds (${cfg.lockDuration.toNumber() / 86400} days)`);
    console.log(`    Multiplier:   ${cfg.multiplier} (${cfg.multiplier / 100}x)`);

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
        layerTokenMint: cfg.tokenMintB, // CRITICAL: must equal whirlpool's token_b
        systemProgram: SystemProgram.programId,
      })
      .preInstructions(computeBudgetIx())
      .rpc();

    await connection.confirmTransaction(sig, "confirmed");
    txSignatures.push({ step: `Configure Level ${cfg.level}`, signature: sig });
    console.log(`    [OK]`, explorerLink(sig));
  }

  // =========================================================================
  // STEP 6: Verification
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("STEP 6: Verification");
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
  console.log("    lockDuration:        ", configData.lockDuration.toString(), "seconds");
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

  let allValid = true;
  for (const cfg of LEVEL_CONFIGS) {
    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([cfg.level])],
      EVOLUTION_PROGRAM_ID,
    );
    const levelData = await evoAccounts.levelConfig.fetch(levelConfigPda);

    const tokenBMatch = levelData.tokenMintB.toBase58() === cfg.tokenMintB.toBase58();
    const layerMatch = levelData.layerTokenMint.toBase58() === cfg.tokenMintB.toBase58();
    const valid = tokenBMatch && layerMatch;
    if (!valid) allValid = false;

    console.log(`\n  Level ${cfg.level} Config: ${valid ? "[OK]" : "[MISMATCH]"}`);
    console.log("    whirlpool:     ", levelData.whirlpool.toBase58());
    console.log("    tokenMintA:    ", levelData.tokenMintA.toBase58());
    console.log("    tokenMintB:    ", levelData.tokenMintB.toBase58(), tokenBMatch ? "" : " [EXPECTED: " + cfg.tokenMintB.toBase58() + "]");
    console.log("    layerTokenMint:", levelData.layerTokenMint.toBase58(), layerMatch ? "" : " [EXPECTED: " + cfg.tokenMintB.toBase58() + "]");
    console.log("    requiredAmountA:", levelData.requiredAmountA.toString());
    console.log("    requiredAmountB:", levelData.requiredAmountB.toString());
    console.log("    feeBps:        ", levelData.feeBps);
    console.log("    lockDuration:  ", levelData.lockDuration.toString(), "seconds");
    console.log("    multiplier:    ", levelData.multiplier);
    console.log("    isActive:      ", levelData.isActive);
  }

  // =========================================================================
  // SUMMARY
  // =========================================================================

  console.log("\n" + "=".repeat(70));
  console.log(allValid ? "CONFIGURATION COMPLETE" : "CONFIGURATION COMPLETE (WITH WARNINGS)");
  console.log("=".repeat(70));

  console.log("\nProgram IDs:");
  console.log("  LP Bonds:   ", LP_BONDS_PROGRAM_ID.toBase58());
  console.log("  Evolution:  ", EVOLUTION_PROGRAM_ID.toBase58());

  console.log("\nAdmin:        ", admin.publicKey.toBase58());

  console.log("\nPDAs:");
  console.log("  Config:              ", configPda.toBase58());
  console.log("  Bond Authority:      ", bondAuthorityPda.toBase58());
  console.log("  Oracle Config:       ", oracleConfigPda.toBase58());
  console.log("  Evolution Config:    ", evolutionConfigPda.toBase58());
  console.log("  Layer Token Auth:    ", layerTokenAuthorityPda.toBase58());

  console.log("\nLevel Configuration:");
  for (const cfg of LEVEL_CONFIGS) {
    console.log(`  Level ${cfg.level}: whirlpool=${cfg.whirlpool.toBase58().slice(0, 8)}... tokenB=${cfg.tokenMintB.toBase58().slice(0, 8)}...`);
  }

  if (txSignatures.length > 0) {
    console.log("\nTransaction Log:");
    for (const { step, signature } of txSignatures) {
      console.log(`  ${step}: ${signature}`);
    }
    console.log("\nTotal transactions:", txSignatures.length);
  } else {
    console.log("\nNo new transactions (all accounts already initialized).");
  }

  if (!allValid) {
    console.log("\n[WARNING] Some level configurations have mismatched token addresses.");
    console.log("This can cause evolution failures. Rerun this script to fix.");
  }

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
