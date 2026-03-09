/**
 * Devnet execution script for LP Bonds Protocol
 * 
 * Calls add_liquidity_and_mint_bond instruction on deployed program.
 * 
 * Program ID: AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w
 * Whirlpool:  8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN
 * 
 * Prerequisites:
 * - Protocol config must be initialized (run initialize first if not)
 * - User must have devnet SOL
 * - User must have token B in their wallet
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { LpBonds } from "../target/types/lp_bonds";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  ComputeBudgetProgram,
  Connection,
} from "@solana/web3.js";
import {
  NATIVE_MINT,
  getAssociatedTokenAddress,
  getOrCreateAssociatedTokenAccount,
} from "@solana/spl-token";

// =============================================================================
// CONSTANTS
// =============================================================================

const PROGRAM_ID = new PublicKey("AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w");
const WHIRLPOOL_PROGRAM_ID = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
const WHIRLPOOL = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");
const TOKEN_MINT_B = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

// Seeds from program
const CONFIG_SEED = Buffer.from("config");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const POSITION_CUSTODY_SEED = Buffer.from("position_custody");

// Whirlpool tick array size
const TICK_ARRAY_SIZE = 88;

// =============================================================================
// WHIRLPOOL ACCOUNT LAYOUT (BORSH)
// =============================================================================

interface WhirlpoolData {
  whirlpoolsConfig: PublicKey;
  whirlpoolBump: number[];
  tickSpacing: number;
  tickSpacingSeed: number[];
  feeRate: number;
  protocolFeeRate: number;
  liquidity: BN;
  sqrtPrice: BN;
  tickCurrentIndex: number;
  protocolFeeOwedA: BN;
  protocolFeeOwedB: BN;
  tokenMintA: PublicKey;
  tokenVaultA: PublicKey;
  feeGrowthGlobalA: BN;
  tokenMintB: PublicKey;
  tokenVaultB: PublicKey;
  feeGrowthGlobalB: BN;
}

/**
 * Parse Whirlpool account data manually since we need vault addresses
 */
function parseWhirlpoolAccount(data: Buffer): WhirlpoolData {
  // Discriminator: 8 bytes
  let offset = 8;
  
  // whirlpools_config: Pubkey (32)
  const whirlpoolsConfig = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  
  // whirlpool_bump: [u8; 1]
  const whirlpoolBump = [data[offset]];
  offset += 1;
  
  // tick_spacing: u16
  const tickSpacing = data.readUInt16LE(offset);
  offset += 2;
  
  // tick_spacing_seed: [u8; 2]
  const tickSpacingSeed = [data[offset], data[offset + 1]];
  offset += 2;
  
  // fee_rate: u16
  const feeRate = data.readUInt16LE(offset);
  offset += 2;
  
  // protocol_fee_rate: u16
  const protocolFeeRate = data.readUInt16LE(offset);
  offset += 2;
  
  // liquidity: u128 (16 bytes)
  const liquidity = new BN(data.subarray(offset, offset + 16), "le");
  offset += 16;
  
  // sqrt_price: u128 (16 bytes)
  const sqrtPrice = new BN(data.subarray(offset, offset + 16), "le");
  offset += 16;
  
  // tick_current_index: i32
  const tickCurrentIndex = data.readInt32LE(offset);
  offset += 4;
  
  // protocol_fee_owed_a: u64
  const protocolFeeOwedA = new BN(data.subarray(offset, offset + 8), "le");
  offset += 8;
  
  // protocol_fee_owed_b: u64
  const protocolFeeOwedB = new BN(data.subarray(offset, offset + 8), "le");
  offset += 8;
  
  // token_mint_a: Pubkey
  const tokenMintA = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  
  // token_vault_a: Pubkey
  const tokenVaultA = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  
  // fee_growth_global_a: u128
  const feeGrowthGlobalA = new BN(data.subarray(offset, offset + 16), "le");
  offset += 16;
  
  // token_mint_b: Pubkey
  const tokenMintB = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  
  // token_vault_b: Pubkey
  const tokenVaultB = new PublicKey(data.subarray(offset, offset + 32));
  offset += 32;
  
  // fee_growth_global_b: u128
  const feeGrowthGlobalB = new BN(data.subarray(offset, offset + 16), "le");
  offset += 16;
  
  return {
    whirlpoolsConfig,
    whirlpoolBump,
    tickSpacing,
    tickSpacingSeed,
    feeRate,
    protocolFeeRate,
    liquidity,
    sqrtPrice,
    tickCurrentIndex,
    protocolFeeOwedA,
    protocolFeeOwedB,
    tokenMintA,
    tokenVaultA,
    feeGrowthGlobalA,
    tokenMintB,
    tokenVaultB,
    feeGrowthGlobalB,
  };
}

// =============================================================================
// PDA DERIVATION FUNCTIONS
// =============================================================================

function deriveConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
}

function deriveBondAuthorityPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([BOND_AUTHORITY_SEED], PROGRAM_ID);
}

function derivePositionCustodyPda(bondMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [POSITION_CUSTODY_SEED, bondMint.toBuffer()],
    PROGRAM_ID
  );
}

function deriveWhirlpoolPositionPda(positionMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("position"), positionMint.toBuffer()],
    WHIRLPOOL_PROGRAM_ID
  );
}

/**
 * Derive tick array PDA for a given tick index.
 * 
 * Tick arrays contain TICK_ARRAY_SIZE (88) ticks.
 * The start tick index must be aligned to tickSpacing * TICK_ARRAY_SIZE.
 */
function deriveTickArrayPda(whirlpool: PublicKey, tickIndex: number, tickSpacing: number): [PublicKey, number] {
  const ticksInArray = tickSpacing * TICK_ARRAY_SIZE;
  const startTickIndex = Math.floor(tickIndex / ticksInArray) * ticksInArray;
  
  return PublicKey.findProgramAddressSync(
    [
      Buffer.from("tick_array"),
      whirlpool.toBuffer(),
      Buffer.from(startTickIndex.toString()),
    ],
    WHIRLPOOL_PROGRAM_ID
  );
}

// =============================================================================
// MAIN EXECUTION
// =============================================================================

async function main() {
  console.log("═══════════════════════════════════════════════════════════════");
  console.log("LP BONDS PROTOCOL - DEVNET LIQUIDITY ADDITION");
  console.log("═══════════════════════════════════════════════════════════════\n");

  // Setup connection to devnet
  const connection = new Connection("https://api.devnet.solana.com", "confirmed");
  
  // Load wallet from keypair file
  const fs = require("fs");
  const os = require("os");
  const walletPath = process.env.ANCHOR_WALLET || "~/.config/solana/id.json";
  const resolvedPath = walletPath.replace("~", os.homedir());
  const keypairData = JSON.parse(fs.readFileSync(resolvedPath, "utf-8"));
  const user = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  const wallet = new anchor.Wallet(user);
  
  // Create provider
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
  });
  anchor.setProvider(provider);

  console.log("Network:    devnet");
  console.log("Program ID:", PROGRAM_ID.toBase58());
  console.log("Whirlpool: ", WHIRLPOOL.toBase58());
  console.log("User:      ", user.publicKey.toBase58());

  // Load program
  const idl = require("../target/idl/lp_bonds.json");
  const program = new Program<LpBonds>(idl, provider);

  // Check user balance
  const balance = await connection.getBalance(user.publicKey);
  console.log(`\nUser SOL Balance: ${balance / LAMPORTS_PER_SOL} SOL`);
  
  if (balance < 0.05 * LAMPORTS_PER_SOL) {
    throw new Error("Insufficient SOL balance. Need at least 0.05 SOL for transaction fees and wrapping.");
  }

  // =========================================================================
  // STEP 1: FETCH AND PARSE WHIRLPOOL DATA
  // =========================================================================

  console.log("\n─── Fetching Whirlpool Data ───");

  const whirlpoolAccountInfo = await connection.getAccountInfo(WHIRLPOOL);
  if (!whirlpoolAccountInfo) {
    throw new Error("Whirlpool account not found on devnet");
  }

  const whirlpoolData = parseWhirlpoolAccount(whirlpoolAccountInfo.data);
  
  console.log("Token Mint A (wSOL):", whirlpoolData.tokenMintA.toBase58());
  console.log("Token Mint B:       ", whirlpoolData.tokenMintB.toBase58());
  console.log("Token Vault A:      ", whirlpoolData.tokenVaultA.toBase58());
  console.log("Token Vault B:      ", whirlpoolData.tokenVaultB.toBase58());
  console.log("Tick Spacing:       ", whirlpoolData.tickSpacing);
  console.log("Current Tick:       ", whirlpoolData.tickCurrentIndex);
  console.log("Liquidity:          ", whirlpoolData.liquidity.toString());

  // Validate token ordering
  if (!whirlpoolData.tokenMintA.equals(NATIVE_MINT)) {
    throw new Error(`Token A is not wSOL. Got: ${whirlpoolData.tokenMintA.toBase58()}`);
  }
  if (!whirlpoolData.tokenMintB.equals(TOKEN_MINT_B)) {
    throw new Error(`Token B mismatch. Got: ${whirlpoolData.tokenMintB.toBase58()}`);
  }

  const tickSpacing = whirlpoolData.tickSpacing;

  // =========================================================================
  // STEP 2: DERIVE ALL PDAs
  // =========================================================================

  console.log("\n─── Deriving PDAs ───");

  const [configPda, _configBump] = deriveConfigPda();
  const [bondAuthorityPda, _bondAuthorityBump] = deriveBondAuthorityPda();
  
  console.log("Config PDA:        ", configPda.toBase58());
  console.log("Bond Authority PDA:", bondAuthorityPda.toBase58());

  // Check if protocol is initialized
  const configAccount = await connection.getAccountInfo(configPda);
  if (!configAccount) {
    console.log("\n⚠️  Protocol not initialized. Initializing...\n");
    
    const initTx = await program.methods
      .initialize()
      .accountsPartial({
        admin: user.publicKey,
        config: configPda,
        bondAuthority: bondAuthorityPda,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
    
    console.log("✓ Protocol initialized:", initTx);
    console.log("  https://explorer.solana.com/tx/" + initTx + "?cluster=devnet\n");
    
    // Wait for confirmation
    await connection.confirmTransaction(initTx, "confirmed");
  } else {
    console.log("✓ Protocol already initialized");
  }

  // =========================================================================
  // STEP 3: GENERATE KEYPAIRS FOR NEW MINTS
  // =========================================================================

  console.log("\n─── Generating Keypairs ───");

  const bondMint = Keypair.generate();
  const positionMint = Keypair.generate();
  
  console.log("Bond Mint:     ", bondMint.publicKey.toBase58());
  console.log("Position Mint: ", positionMint.publicKey.toBase58());

  // Derive remaining PDAs that depend on generated keypairs
  const [positionCustodyPda, _positionCustodyBump] = derivePositionCustodyPda(bondMint.publicKey);
  const [whirlpoolPositionPda, _positionBump] = deriveWhirlpoolPositionPda(positionMint.publicKey);
  
  console.log("Position Custody PDA:", positionCustodyPda.toBase58());
  console.log("Whirlpool Position:  ", whirlpoolPositionPda.toBase58());

  // =========================================================================
  // STEP 4: CALCULATE TICK RANGE AND DERIVE TICK ARRAYS
  // =========================================================================

  console.log("\n─── Calculating Tick Range ───");

  // Use current tick as center, create range around it
  // Align to tick spacing
  const currentTick = whirlpoolData.tickCurrentIndex;
  const tickRange = tickSpacing * 10; // 10 tick spacings on each side
  
  const tickLowerIndex = Math.floor((currentTick - tickRange) / tickSpacing) * tickSpacing;
  const tickUpperIndex = Math.ceil((currentTick + tickRange) / tickSpacing) * tickSpacing;
  
  console.log("Tick Lower Index:", tickLowerIndex);
  console.log("Tick Upper Index:", tickUpperIndex);
  console.log("Current Tick:    ", currentTick);

  // Derive tick array PDAs
  const [tickArrayLowerPda] = deriveTickArrayPda(WHIRLPOOL, tickLowerIndex, tickSpacing);
  const [tickArrayUpperPda] = deriveTickArrayPda(WHIRLPOOL, tickUpperIndex, tickSpacing);
  
  console.log("Tick Array Lower:", tickArrayLowerPda.toBase58());
  console.log("Tick Array Upper:", tickArrayUpperPda.toBase58());

  // Verify tick arrays exist - FAIL HARD if not
  const tickArrayLowerInfo = await connection.getAccountInfo(tickArrayLowerPda);
  const tickArrayUpperInfo = await connection.getAccountInfo(tickArrayUpperPda);
  
  if (!tickArrayLowerInfo || !tickArrayUpperInfo) {
    console.error("\n❌ FATAL: Tick arrays not initialized for selected range.");
    console.error("   Lower tick array exists:", !!tickArrayLowerInfo);
    console.error("   Upper tick array exists:", !!tickArrayUpperInfo);
    console.error("\n   You need to either:");
    console.error("   1. Initialize tick arrays using Orca SDK, or");
    console.error("   2. Choose a tick range with existing tick arrays.");
    throw new Error("Tick arrays not initialized. Cannot proceed.");
  }
  console.log("✓ Both tick arrays exist");

  // =========================================================================
  // STEP 5: DERIVE USER TOKEN ACCOUNTS
  // =========================================================================

  console.log("\n─── Deriving User Token Accounts ───");

  const userWsolAccount = Keypair.generate(); // Temporary, created inline by program
  const userTokenBAccountAddress = await getAssociatedTokenAddress(TOKEN_MINT_B, user.publicKey);
  const userBondAccount = await getAssociatedTokenAddress(bondMint.publicKey, user.publicKey);
  
  // Position token account owned by USER (open_position creates this for owner)
  const positionTokenAccount = await getAssociatedTokenAddress(
    positionMint.publicKey,
    user.publicKey
  );
  
  // Custody position token account owned by position_custody PDA
  // Position NFT will be transferred here after liquidity is added
  const custodyPositionTokenAccount = await getAssociatedTokenAddress(
    positionMint.publicKey,
    positionCustodyPda,
    true // allowOwnerOffCurve for PDA
  );
  
  console.log("User wSOL Account (temp):", userWsolAccount.publicKey.toBase58());
  console.log("User Token B Account:    ", userTokenBAccountAddress.toBase58());
  console.log("User Bond Account:       ", userBondAccount.toBase58());
  console.log("Position Token Account:  ", positionTokenAccount.toBase58(), "(user's ATA)");
  console.log("Custody Position ATA:    ", custodyPositionTokenAccount.toBase58(), "(custody PDA's ATA)");

  // Ensure Token B ATA exists (program requires it even if token_max_b = 0)
  console.log("\n─── Ensuring Token B ATA Exists ───");
  let userTokenBAccountInfo;
  try {
    userTokenBAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      user,
      TOKEN_MINT_B,
      user.publicKey
    );
    console.log("✓ Token B ATA ready:", userTokenBAccountInfo.address.toBase58());
    console.log("  Balance:", userTokenBAccountInfo.amount.toString(), "tokens");
  } catch (e: any) {
    console.error("❌ Failed to create/fetch Token B ATA:", e.message);
    throw new Error("Cannot proceed without Token B ATA");
  }

  // =========================================================================
  // STEP 6: PREPARE INSTRUCTION PARAMETERS
  // =========================================================================

  console.log("\n─── Preparing Transaction ───");

  // =========================================================================
  // LIQUIDITY CALCULATION
  // =========================================================================
  // Whirlpool uses Q64.64 fixed-point sqrt_price
  // sqrt_price = sqrt(1.0001^tick) × 2^64
  // 
  // For a two-sided position (current tick in range):
  //   amount_a = L × (1/sqrt_current - 1/sqrt_upper)
  //   amount_b = L × (sqrt_current - sqrt_lower)
  //
  // Solving for L from desired amount_a:
  //   L = amount_a × sqrt_current × sqrt_upper / (sqrt_upper - sqrt_current)
  // =========================================================================

  function tickToSqrtPriceX64(tick: number): BN {
    // sqrt_price = sqrt(1.0001^tick) × 2^64
    // = 1.0001^(tick/2) × 2^64
    const sqrtPriceDecimal = Math.pow(1.0001, tick / 2);
    // Be careful with precision - use string conversion for large numbers
    return new BN(Math.floor(sqrtPriceDecimal * Math.pow(2, 64)).toString());
  }

  const sqrtPriceLower = tickToSqrtPriceX64(tickLowerIndex);
  const sqrtPriceUpper = tickToSqrtPriceX64(tickUpperIndex);
  const sqrtPriceCurrent = whirlpoolData.sqrtPrice;

  console.log("Sqrt Price Lower:  ", sqrtPriceLower.toString());
  console.log("Sqrt Price Upper:  ", sqrtPriceUpper.toString());
  console.log("Sqrt Price Current:", sqrtPriceCurrent.toString());

  // Desired SOL amount to deposit
  const desiredSolAmount = new BN(0.001 * LAMPORTS_PER_SOL); // 0.001 SOL for testing

  // Calculate liquidity from desired amount_a (SOL)
  // L = amount_a × sqrt_current × sqrt_upper / (sqrt_upper - sqrt_current)
  let liquidityAmount: BN;
  let expectedAmountA: BN;
  let expectedAmountB: BN;

  const q64 = new BN(2).pow(new BN(64));

  if (tickLowerIndex > currentTick) {
    // Single-sided: Only Token A (price below range - deposit only SOL)
    // L = amount_a × sqrt_lower × sqrt_upper / (sqrt_upper - sqrt_lower)
    liquidityAmount = desiredSolAmount
      .mul(sqrtPriceLower)
      .mul(sqrtPriceUpper)
      .div(sqrtPriceUpper.sub(sqrtPriceLower))
      .div(q64);
    expectedAmountA = desiredSolAmount;
    expectedAmountB = new BN(0);
  } else if (tickUpperIndex < currentTick) {
    // Single-sided: Only Token B (price above range)
    // L = amount_b × 2^64 / (sqrt_upper - sqrt_lower)
    // We'd need to calculate from desired Token B instead
    liquidityAmount = new BN(1000); // minimal for testing
    expectedAmountA = new BN(0);
    expectedAmountB = liquidityAmount.mul(sqrtPriceUpper.sub(sqrtPriceLower)).div(q64);
  } else {
    // Two-sided: Both tokens needed (current tick in range)
    // L = amount_a × sqrt_current × sqrt_upper / (sqrt_upper - sqrt_current)
    liquidityAmount = desiredSolAmount
      .mul(sqrtPriceCurrent)
      .mul(sqrtPriceUpper)
      .div(sqrtPriceUpper.sub(sqrtPriceCurrent))
      .div(q64);
    
    // Calculate expected amounts
    // amount_a = L × (sqrt_upper - sqrt_current) × 2^64 / (sqrt_current × sqrt_upper)
    // amount_b = L × (sqrt_current - sqrt_lower) / 2^64
    expectedAmountA = liquidityAmount
      .mul(sqrtPriceUpper.sub(sqrtPriceCurrent))
      .div(sqrtPriceCurrent.mul(sqrtPriceUpper).div(q64));
    expectedAmountB = liquidityAmount
      .mul(sqrtPriceCurrent.sub(sqrtPriceLower))
      .div(q64);
  }

  console.log("Calculated Liquidity:", liquidityAmount.toString());
  console.log("Expected Amount A:   ", expectedAmountA.toString(), "lamports");
  console.log("Expected Amount B:   ", expectedAmountB.toString(), "tokens");

  // Set token max with 20% slippage buffer
  const tokenMaxA = expectedAmountA.muln(120).divn(100).add(new BN(10000)); // +20% + dust
  const tokenMaxB = expectedAmountB.muln(120).divn(100).add(new BN(10000)); // +20% + dust
  const solAmount = tokenMaxA; // Wrap enough SOL to cover max

  // Validate user has enough Token B
  const userTokenBBalance = userTokenBAccountInfo.amount;
  if (expectedAmountB.gtn(0) && BigInt(expectedAmountB.toString()) > userTokenBBalance) {
    throw new Error(`Insufficient Token B: need ${expectedAmountB.toString()}, have ${userTokenBBalance.toString()}`);
  }
  
  // Determine position type
  let positionType: string;
  if (tickLowerIndex > currentTick) {
    positionType = "SINGLE-SIDED (Token A / wSOL only - price below range)";
  } else if (tickUpperIndex < currentTick) {
    positionType = "SINGLE-SIDED (Token B only - price above range)";
  } else {
    positionType = "TWO-SIDED (Both tokens required - price in range)";
  }

  console.log("Position Type:    ", positionType);
  console.log("SOL Amount:       ", solAmount.toString(), "lamports (", solAmount.toNumber() / LAMPORTS_PER_SOL, "SOL)");
  console.log("Liquidity Amount: ", liquidityAmount.toString());
  console.log("Token Max A:      ", tokenMaxA.toString(), "lamports");
  console.log("Token Max B:      ", tokenMaxB.toString(), "tokens");
  console.log("User Token B bal: ", userTokenBBalance.toString());
  
  // Warning if two-sided but no token B
  if (positionType.includes("TWO-SIDED") && tokenMaxB.isZero()) {
    console.warn("\n⚠️  WARNING: Range includes current price but you have no Token B.");
    console.warn("   Whirlpool may reject with TokenMaxExceeded error.");
    console.warn("   Consider: 1) Getting Token B, or 2) Using a single-sided range.\n");
  }

  // =========================================================================
  // STEP 7: BUILD AND SEND TRANSACTION
  // =========================================================================

  console.log("\n─── Sending Transaction ───");

  try {
    const tx = await program.methods
      .addLiquidityAndMintBond(
        tickLowerIndex,
        tickUpperIndex,
        liquidityAmount,
        tokenMaxA,
        tokenMaxB,
        solAmount
      )
      .accountsPartial({
        // User accounts
        user: user.publicKey,
        
        // Token mints - MUST be explicitly provided
        wsolMint: NATIVE_MINT,
        tokenMintB: TOKEN_MINT_B,
        bondAuthority: bondAuthorityPda,
        
        // Bond NFT accounts
        bondMint: bondMint.publicKey,
        
        // User token accounts
        userWsolAccount: userWsolAccount.publicKey,
        userTokenBAccount: userTokenBAccountAddress,
        userBondAccount: userBondAccount,
        
        // Protocol accounts
        config: configPda,
        positionCustody: positionCustodyPda,
        
        // Whirlpool position accounts
        positionMint: positionMint.publicKey,
        whirlpoolPosition: whirlpoolPositionPda,
        positionTokenAccount: positionTokenAccount,
        custodyPositionTokenAccount: custodyPositionTokenAccount,
        
        // Whirlpool accounts
        whirlpool: WHIRLPOOL,
        tokenVaultA: whirlpoolData.tokenVaultA,
        tokenVaultB: whirlpoolData.tokenVaultB,
        tickArrayLower: tickArrayLowerPda,
        tickArrayUpper: tickArrayUpperPda,
        
        // Only specify non-auto-derivable programs
        whirlpoolProgram: WHIRLPOOL_PROGRAM_ID,
        // Let Anchor auto-resolve: tokenProgram, associatedTokenProgram, systemProgram, rent
      })
      .preInstructions([
        // Increased compute budget for: open_position CPI + increase_liquidity CPI + create_metadata_accounts_v3
        ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }),
      ])
      .signers([bondMint, positionMint, userWsolAccount])
      .rpc();

    console.log("\n═══════════════════════════════════════════════════════════════");
    console.log("✓ TRANSACTION SUCCESSFUL");
    console.log("═══════════════════════════════════════════════════════════════");
    console.log("\nSignature:", tx);
    console.log("\nExplorer Link:");
    console.log(`https://explorer.solana.com/tx/${tx}?cluster=devnet`);
    console.log("\nBond NFT Mint:", bondMint.publicKey.toBase58());
    console.log("Position Mint:", positionMint.publicKey.toBase58());
    console.log("═══════════════════════════════════════════════════════════════\n");

  } catch (error: any) {
    console.error("\n═══════════════════════════════════════════════════════════════");
    console.error("✗ TRANSACTION FAILED");
    console.error("═══════════════════════════════════════════════════════════════");
    
    if (error.logs) {
      console.error("\nProgram Logs:");
      error.logs.forEach((log: string) => console.error("  ", log));
    }
    
    if (error.message) {
      console.error("\nError Message:", error.message);
    }
    
    console.error("═══════════════════════════════════════════════════════════════\n");
    throw error;
  }
}

main().catch(console.error);
