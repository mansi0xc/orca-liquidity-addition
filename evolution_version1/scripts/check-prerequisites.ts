/**
 * Check all prerequisites for running devnet-add-liquidity.ts
 */

import { PublicKey, Connection } from "@solana/web3.js";
import { getAssociatedTokenAddress, getAccount } from "@solana/spl-token";

const WHIRLPOOL = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");
const WHIRLPOOL_PROGRAM_ID = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
const TOKEN_MINT_B = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");
const TICK_ARRAY_SIZE = 88;

async function main() {
  const connection = new Connection("https://api.devnet.solana.com", "confirmed");
  
  console.log("═══════════════════════════════════════════════════════════════");
  console.log("      LP BONDS PROTOCOL - PREREQUISITES CHECK");
  console.log("═══════════════════════════════════════════════════════════════\n");

  // 1. Check whirlpool exists
  console.log("1️⃣  WHIRLPOOL CHECK");
  const whirlpoolInfo = await connection.getAccountInfo(WHIRLPOOL);
  if (!whirlpoolInfo) {
    console.log("   ❌ Whirlpool not found!");
    return;
  }
  console.log("   ✓ Whirlpool exists");
  
  // Parse whirlpool data
  const data = whirlpoolInfo.data;
  let offset = 8 + 32 + 1;
  const tickSpacing = data.readUInt16LE(offset);
  offset += 2 + 2 + 2 + 2 + 16 + 16;
  const currentTick = data.readInt32LE(offset);
  
  console.log(`   Tick Spacing: ${tickSpacing}`);
  console.log(`   Current Tick: ${currentTick}`);

  // 2. Calculate proposed tick range
  console.log("\n2️⃣  TICK RANGE");
  const tickRange = tickSpacing * 10;
  const tickLower = Math.floor((currentTick - tickRange) / tickSpacing) * tickSpacing;
  const tickUpper = Math.ceil((currentTick + tickRange) / tickSpacing) * tickSpacing;
  
  console.log(`   Proposed Lower: ${tickLower}`);
  console.log(`   Proposed Upper: ${tickUpper}`);
  
  // Determine position type
  let positionType: string;
  if (tickLower > currentTick) {
    positionType = "SINGLE-SIDED (wSOL only)";
  } else if (tickUpper < currentTick) {
    positionType = "SINGLE-SIDED (Token B only)";
  } else {
    positionType = "TWO-SIDED (Both tokens needed)";
  }
  console.log(`   Position Type: ${positionType}`);

  // 3. Check tick arrays
  console.log("\n3️⃣  TICK ARRAYS");
  const ticksInArray = tickSpacing * TICK_ARRAY_SIZE;
  const startLower = Math.floor(tickLower / ticksInArray) * ticksInArray;
  const startUpper = Math.floor(tickUpper / ticksInArray) * ticksInArray;
  
  const [arrLower] = PublicKey.findProgramAddressSync(
    [Buffer.from("tick_array"), WHIRLPOOL.toBuffer(), Buffer.from(startLower.toString())],
    WHIRLPOOL_PROGRAM_ID
  );
  const [arrUpper] = PublicKey.findProgramAddressSync(
    [Buffer.from("tick_array"), WHIRLPOOL.toBuffer(), Buffer.from(startUpper.toString())],
    WHIRLPOOL_PROGRAM_ID
  );
  
  const lowerExists = await connection.getAccountInfo(arrLower);
  const upperExists = await connection.getAccountInfo(arrUpper);
  
  console.log(`   Lower Array (start: ${startLower}): ${lowerExists ? "✓ EXISTS" : "❌ NOT INITIALIZED"}`);
  console.log(`   Upper Array (start: ${startUpper}): ${upperExists ? "✓ EXISTS" : "❌ NOT INITIALIZED"}`);
  
  if (!lowerExists || !upperExists) {
    console.log("\n   ⚠️  TICK ARRAYS MISSING - You need to initialize them first!");
    console.log("   Run: scripts/initialize-tick-arrays.ts (to be created)");
  }

  // 4. Get user wallet
  console.log("\n4️⃣  WALLET CHECK");
  const walletPath = process.env.ANCHOR_WALLET || "~/.config/solana/id.json";
  const fs = require("fs");
  const os = require("os");
  const resolvedPath = walletPath.replace("~", os.homedir());
  
  try {
    const keypairData = JSON.parse(fs.readFileSync(resolvedPath, "utf-8"));
    const { Keypair } = require("@solana/web3.js");
    const wallet = Keypair.fromSecretKey(Uint8Array.from(keypairData));
    
    console.log(`   Wallet: ${wallet.publicKey.toBase58()}`);
    
    const solBalance = await connection.getBalance(wallet.publicKey);
    console.log(`   SOL Balance: ${solBalance / 1e9} SOL ${solBalance < 0.05e9 ? "❌ NEED MORE SOL" : "✓"}`);
    
    // Check Token B balance
    const tokenBAddress = await getAssociatedTokenAddress(TOKEN_MINT_B, wallet.publicKey);
    try {
      const tokenBAccount = await getAccount(connection, tokenBAddress);
      console.log(`   Token B Balance: ${tokenBAccount.amount.toString()} ✓`);
    } catch {
      console.log(`   Token B Balance: 0 (ATA will be created)`);
    }
  } catch (e: any) {
    console.log(`   ❌ Could not read wallet: ${e.message}`);
  }

  // Summary
  console.log("\n═══════════════════════════════════════════════════════════════");
  console.log("SUMMARY");
  console.log("═══════════════════════════════════════════════════════════════");
  
  const allGood = lowerExists && upperExists;
  if (allGood) {
    console.log("✓ All prerequisites met! You can run:");
    console.log("  npx ts-node scripts/devnet-add-liquidity.ts");
  } else {
    console.log("❌ Prerequisites not met:");
    if (!lowerExists || !upperExists) {
      console.log("   - Tick arrays need to be initialized");
    }
  }
  console.log("");
}

main().catch(console.error);
