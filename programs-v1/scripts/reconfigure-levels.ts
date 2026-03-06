/**
 * Reconfigure Evolution Levels
 *
 * Sets layer_token_mint = whirlpool token_b for each level.
 * The protocol mints these internally during evolution (EVM parity).
 * After running this, run transfer-mint-authority.ts to transfer mint
 * authority of these tokens to the layer_token_authority PDA.
 *
 * Safe to run multiple times.
 *
 * Usage:
 *   npx ts-node --project tsconfig.scripts.json scripts/reconfigure-levels.ts
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

const LEVEL_CONFIGS = [
  {
    level: 2,
    whirlpool: new PublicKey("36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V"),
    tokenMintA: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
    tokenMintB: new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB"),
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
    whirlpool: new PublicKey("GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ"),
    tokenMintA: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
    tokenMintB: new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp"),
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
    whirlpool: new PublicKey("2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo"),
    tokenMintA: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
    tokenMintB: new PublicKey("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu"),
    tickLower: -443520,
    tickUpper: 443520,
    requiredAmountA: new BN(4_000_000_000),
    requiredAmountB: new BN(2_000_000_000),
    feeBps: 200,
    lockDuration: new BN(10_368_000),
    multiplier: 300,
  },
];

const EVOLUTION_CONFIG_SEED = Buffer.from("evolution_config");
const LEVEL_CONFIG_SEED = Buffer.from("level_config");

async function main() {
  console.log("=".repeat(70));
  console.log("RECONFIGURE LEVELS - Fix layer_token_mint = whirlpool token_b");
  console.log("=".repeat(70));

  const walletPath = process.env.ANCHOR_WALLET || "~/.config/solana/id.json";
  const resolvedPath = walletPath.replace("~", os.homedir());
  const keypairData = JSON.parse(fs.readFileSync(resolvedPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const rpcUrl = process.env.ANCHOR_PROVIDER_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");
  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  const evolutionIdl = JSON.parse(fs.readFileSync("./target/idl/lp_bonds_evolution.json", "utf-8"));
  const evolutionProgram = new Program(evolutionIdl, provider);
  const EVOLUTION_PROGRAM_ID = evolutionProgram.programId;

  const [evolutionConfigPda] = PublicKey.findProgramAddressSync([EVOLUTION_CONFIG_SEED], EVOLUTION_PROGRAM_ID);

  console.log("\nAdmin:", admin.publicKey.toBase58());
  console.log("Evolution Program:", EVOLUTION_PROGRAM_ID.toBase58());

  for (const cfg of LEVEL_CONFIGS) {
    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([cfg.level])],
      EVOLUTION_PROGRAM_ID,
    );

    console.log(`\nReconfiguring Level ${cfg.level}:`);
    console.log(`  layer_token_mint = token_b: ${cfg.tokenMintB.toBase58()}`);

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
        true,
      )
      .accountsPartial({
        admin: admin.publicKey,
        evolutionConfig: evolutionConfigPda,
        levelConfig: levelConfigPda,
        whirlpool: cfg.whirlpool,
        tokenMintA: cfg.tokenMintA,
        tokenMintB: cfg.tokenMintB,
        layerTokenMint: cfg.tokenMintB, // KEY FIX: layer_token = whirlpool token_b
        systemProgram: SystemProgram.programId,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: 400_000 }),
        ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 5_000 }),
      ])
      .rpc();

    await connection.confirmTransaction(sig, "confirmed");
    console.log(`  Done: https://explorer.solana.com/tx/${sig}?cluster=devnet`);
  }

  // Verify
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const evoAccounts = evolutionProgram.account as any;
  console.log("\nVerification:");
  for (const cfg of LEVEL_CONFIGS) {
    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([cfg.level])],
      EVOLUTION_PROGRAM_ID,
    );
    const data = await evoAccounts.levelConfig.fetch(levelConfigPda);
    const match = data.layerTokenMint.toBase58() === cfg.tokenMintB.toBase58();
    console.log(`  Level ${cfg.level}: layerToken=${data.layerTokenMint.toBase58()} ${match ? "OK" : "MISMATCH!"}`);
  }

  console.log("\n" + "=".repeat(70));
  console.log("RECONFIGURATION COMPLETE");
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
