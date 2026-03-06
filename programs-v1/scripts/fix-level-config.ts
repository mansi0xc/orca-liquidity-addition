import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { ComputeBudgetProgram } from "@solana/web3.js";
import * as fs from "fs";

// Hardcoded IDs
const EVOLUTION_PROGRAM_ID = new PublicKey("Bk81YHvFinrSCs64W7MzobDhMJNXrUEqAU5YpAWcotua");
const WHIRLPOOL_L2 = new PublicKey("36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V");

// Whirlpool token mints (from our earlier decode)
const TOKEN_MINT_A = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const TOKEN_MINT_B = new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB"); // Actual whirlpool Token B

const LEVEL_CONFIG_SEED = Buffer.from("level_config");

async function main() {
  // Setup provider
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const wallet = (provider.wallet as anchor.Wallet).payer;

  console.log("Wallet:", wallet.publicKey.toString());
  console.log("Evolution Program:", EVOLUTION_PROGRAM_ID.toString());
  
  // Load IDL
  const idlPath = "./target/idl/lp_bonds_evolution.json";
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
  const evolutionProgram = new Program(idl, provider);

  // Derive PDAs
  const level = 2;
  const [levelConfigPda] = PublicKey.findProgramAddressSync(
    [LEVEL_CONFIG_SEED, Buffer.from([level])],
    EVOLUTION_PROGRAM_ID
  );

  console.log("\nReconfiguring Level", level, "with correct Token B...");
  console.log("  Level Config PDA:", levelConfigPda.toString());
  console.log("  Whirlpool:", WHIRLPOOL_L2.toString());
  console.log("  Token Mint A:", TOKEN_MINT_A.toString());
  console.log("  Token Mint B (actual):", TOKEN_MINT_B.toString());

  // Configure with correct values
  const lockDuration = 5_184_000; // 60 days
  const feeBps = 100; // 1%
  const multiplier = 150; // 1.5x

  const tx = await evolutionProgram.methods
    .configureLevel(
      level,
      -443520, // tick_lower
      443520, // tick_upper
      new BN(2_000_000_000), // required_amount_a (2B = 2 tokens with 9 decimals)
      new BN(1_000_000_000), // required_amount_b (1B = 1 token)
      feeBps,
      new BN(lockDuration),
      multiplier,
      true // is_active
    )
    .accounts({
      admin: wallet.publicKey,
      levelConfig: levelConfigPda,
      whirlpool: WHIRLPOOL_L2,
      tokenMintA: TOKEN_MINT_A,
      tokenMintB: TOKEN_MINT_B, // Use actual whirlpool Token B
      layerTokenMint: TOKEN_MINT_B, // Use same as Token B for now
    })
    .preInstructions([
      ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 5000 }),
    ])
    .rpc();

  console.log("\n✓ Level", level, "reconfigured!");
  console.log("  Tx:", `https://explorer.solana.com/tx/${tx}?cluster=devnet`);

  // Verify by fetching raw account data
  const configAccount = await provider.connection.getAccountInfo(levelConfigPda);
  if (configAccount) {
    const data = configAccount.data;
    let offset = 8 + 1 + 32; // discriminator + level + whirlpool
    const tokenMintA = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const tokenMintB = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
    const layerTokenMint = new PublicKey(data.subarray(offset, offset + 32));
    
    console.log("\nVerified Level Config:");
    console.log("  Token Mint A:", tokenMintA.toString());
    console.log("  Token Mint B:", tokenMintB.toString());
    console.log("  Layer Token Mint:", layerTokenMint.toString());
  }
}

main().catch(console.error);
