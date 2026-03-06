import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { ComputeBudgetProgram } from "@solana/web3.js";
import * as fs from "fs";

const EVOLUTION_PROGRAM_ID = new PublicKey("Bk81YHvFinrSCs64W7MzobDhMJNXrUEqAU5YpAWcotua");
const WHIRLPOOL_L3 = new PublicKey("GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ");

// Whirlpool token mints (from our check)
const TOKEN_MINT_A = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const TOKEN_MINT_B = new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp"); // Actual whirlpool Token B

const LEVEL_CONFIG_SEED = Buffer.from("level_config");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const wallet = (provider.wallet as anchor.Wallet).payer;

  console.log("Wallet:", wallet.publicKey.toString());
  
  const idlPath = "./target/idl/lp_bonds_evolution.json";
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf8"));
  const evolutionProgram = new Program(idl, provider);

  const level = 3;
  const [levelConfigPda] = PublicKey.findProgramAddressSync(
    [LEVEL_CONFIG_SEED, Buffer.from([level])],
    EVOLUTION_PROGRAM_ID
  );

  console.log("\nReconfiguring Level", level, "with correct Token B...");
  console.log("  Level Config PDA:", levelConfigPda.toString());
  console.log("  Whirlpool:", WHIRLPOOL_L3.toString());
  console.log("  Token Mint A:", TOKEN_MINT_A.toString());
  console.log("  Token Mint B (actual):", TOKEN_MINT_B.toString());

  // Level 3: 90 days lock, 1.5% fee, 2x multiplier
  const lockDuration = 7_776_000;
  const feeBps = 150;
  const multiplier = 200;

  const tx = await evolutionProgram.methods
    .configureLevel(
      level,
      -443520,
      443520,
      new BN(3_000_000_000),
      new BN(1_500_000_000),
      feeBps,
      new BN(lockDuration),
      multiplier,
      true
    )
    .accounts({
      admin: wallet.publicKey,
      levelConfig: levelConfigPda,
      whirlpool: WHIRLPOOL_L3,
      tokenMintA: TOKEN_MINT_A,
      tokenMintB: TOKEN_MINT_B,
      layerTokenMint: TOKEN_MINT_B,
    })
    .preInstructions([
      ComputeBudgetProgram.setComputeUnitLimit({ units: 400000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 5000 }),
    ])
    .rpc();

  console.log("\n✓ Level", level, "reconfigured!");
  console.log("  Tx:", `https://explorer.solana.com/tx/${tx}?cluster=devnet`);

  // Verify
  const configAccount = await provider.connection.getAccountInfo(levelConfigPda);
  if (configAccount) {
    const data = configAccount.data;
    let offset = 8 + 1 + 32;
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
