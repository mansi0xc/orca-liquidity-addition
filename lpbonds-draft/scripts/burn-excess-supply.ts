/**
 * Burn Excess Layer Token Supply
 *
 * The existing layer tokens were minted to u64::MAX during devnet setup.
 * The protocol needs to mint layer tokens internally during evolution, so
 * we need headroom in the supply. This script burns tokens from the admin's
 * wallet to make room.
 *
 * Burns enough for ~100 evolutions per level.
 *
 * Usage:
 *   npx ts-node --project tsconfig.scripts.json scripts/burn-excess-supply.ts
 */

import {
  PublicKey,
  Keypair,
  Connection,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  burn,
  getAccount,
} from "@solana/spl-token";
import * as fs from "fs";
import * as os from "os";

const LAYER_TOKENS = [
  {
    level: 2,
    mint: new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB"),
    burnAmount: BigInt("100000000000"), // 100B raw units (100x the required_amount_b of 1B)
  },
  {
    level: 3,
    mint: new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp"),
    burnAmount: BigInt("150000000000"), // 150B raw units (100x the required_amount_b of 1.5B)
  },
  {
    level: 4,
    mint: new PublicKey("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu"),
    burnAmount: BigInt("200000000000"), // 200B raw units (100x the required_amount_b of 2B)
  },
];

async function main() {
  console.log("=".repeat(70));
  console.log("BURN EXCESS LAYER TOKEN SUPPLY");
  console.log("=".repeat(70));

  const walletPath = process.env.ANCHOR_WALLET || "~/.config/solana/id.json";
  const resolvedPath = walletPath.replace("~", os.homedir());
  const keypairData = JSON.parse(fs.readFileSync(resolvedPath, "utf-8"));
  const admin = Keypair.fromSecretKey(Uint8Array.from(keypairData));

  const rpcUrl = process.env.ANCHOR_PROVIDER_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  console.log("\nAdmin:", admin.publicKey.toBase58());

  for (const { level, mint, burnAmount } of LAYER_TOKENS) {
    console.log(`\n--- Level ${level}: ${mint.toBase58()} ---`);

    const ata = await getAssociatedTokenAddress(mint, admin.publicKey);
    const accountInfo = await getAccount(connection, ata);
    console.log(`  Admin balance: ${accountInfo.amount}`);
    console.log(`  Burn amount:   ${burnAmount}`);

    if (accountInfo.amount < burnAmount) {
      console.log("  SKIP: insufficient balance to burn requested amount");
      continue;
    }

    const sig = await burn(
      connection,
      admin,
      ata,
      mint,
      admin,
      burnAmount,
    );

    console.log(`  Burned: ${sig}`);
    console.log(`  Explorer: https://explorer.solana.com/tx/${sig}?cluster=devnet`);

    const after = await getAccount(connection, ata);
    console.log(`  New balance: ${after.amount}`);
  }

  console.log("\n" + "=".repeat(70));
  console.log("BURN COMPLETE");
  console.log("=".repeat(70));
}

main().catch((err) => {
  console.error("\nFATAL ERROR:", err.message || err);
  process.exit(1);
});
