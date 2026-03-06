/**
 * Transfer Mint Authority of Layer Tokens to the Evolution Program PDA
 *
 * The evolution program mints layer tokens internally (EVM parity: curToken1.mint(address(this), ...)).
 * For this to work, the layer_token_authority PDA must be the mint authority of each layer token.
 *
 * This script transfers mint authority from the admin wallet to the PDA for all three
 * whirlpool token_b mints used as layer tokens in evolution levels 2-4.
 *
 * Safe to run multiple times -- skips tokens that already have the correct authority.
 *
 * Usage:
 *   npx ts-node --project tsconfig.scripts.json scripts/transfer-mint-authority.ts
 */

import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  Connection,
} from "@solana/web3.js";
import {
  getMint,
  AuthorityType,
  setAuthority,
} from "@solana/spl-token";
import * as fs from "fs";
import * as os from "os";

const LAYER_TOKEN_AUTHORITY_SEED = Buffer.from("layer_token_authority");

const LAYER_TOKEN_MINTS = [
  { level: 2, mint: new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB") },
  { level: 3, mint: new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp") },
  { level: 4, mint: new PublicKey("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu") },
];

async function main() {
  console.log("=".repeat(70));
  console.log("TRANSFER MINT AUTHORITY TO LAYER TOKEN AUTHORITY PDA");
  console.log("=".repeat(70));

  const walletPath = process.env.ANCHOR_WALLET || "~/.config/solana/id.json";
  const resolvedPath = walletPath.replace("~", os.homedir());
  const keypairData = JSON.parse(fs.readFileSync(resolvedPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const rpcUrl = process.env.ANCHOR_PROVIDER_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  const evolutionIdl = JSON.parse(fs.readFileSync("./target/idl/lp_bonds_evolution.json", "utf-8"));
  const wallet = new anchor.Wallet(admin);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  const evolutionProgram = new Program(evolutionIdl, provider);
  const EVOLUTION_PROGRAM_ID = evolutionProgram.programId;

  const [layerTokenAuthorityPda] = PublicKey.findProgramAddressSync(
    [LAYER_TOKEN_AUTHORITY_SEED],
    EVOLUTION_PROGRAM_ID,
  );

  console.log("\nAdmin:                    ", admin.publicKey.toBase58());
  console.log("Evolution Program:        ", EVOLUTION_PROGRAM_ID.toBase58());
  console.log("Layer Token Authority PDA:", layerTokenAuthorityPda.toBase58());

  for (const { level, mint } of LAYER_TOKEN_MINTS) {
    console.log(`\n--- Level ${level}: ${mint.toBase58()} ---`);

    const mintInfo = await getMint(connection, mint);
    const currentAuthority = mintInfo.mintAuthority;

    console.log(`  Current mint authority: ${currentAuthority?.toBase58() ?? "NONE"}`);

    if (currentAuthority && currentAuthority.equals(layerTokenAuthorityPda)) {
      console.log("  Already set to PDA. Skipping.");
      continue;
    }

    if (!currentAuthority || !currentAuthority.equals(admin.publicKey)) {
      console.error(`  ERROR: Admin does not have mint authority. Current: ${currentAuthority?.toBase58()}`);
      continue;
    }

    const sig = await setAuthority(
      connection,
      admin,
      mint,
      admin,
      AuthorityType.MintTokens,
      layerTokenAuthorityPda,
    );

    console.log(`  Transferred to PDA: ${sig}`);
    console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

    const updated = await getMint(connection, mint);
    console.log(`  Verified new authority: ${updated.mintAuthority?.toBase58()}`);
  }

  console.log("\n" + "=".repeat(70));
  console.log("MINT AUTHORITY TRANSFER COMPLETE");
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
