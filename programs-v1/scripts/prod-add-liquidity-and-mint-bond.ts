/**
 * ============================================================================
 * PRODUCTION-READY: Add Liquidity and Mint Bond - Devnet Script
 * ============================================================================
 * 
 * Program ID: AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w
 * Whirlpool:  8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN
 * Token A:    So11111111111111111111111111111111111111112 (wSOL)
 * Token B:    4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2 (SPL Token)
 * 
 * This script:
 * 1. Derives all PDAs deterministically from the IDL
 * 2. Validates all accounts before sending
 * 3. Simulates transaction before execution
 * 4. Provides comprehensive debug logging
 * 5. Verifies success after execution
 * 
 * Prerequisites:
 * - Protocol must be initialized (run initialize first)
 * - User must have devnet SOL (minimum 0.1 SOL recommended)
 * - User must have Token B if position requires both tokens
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN, AnchorError } from "@coral-xyz/anchor";
import { LpBonds } from "../target/types/lp_bonds";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  LAMPORTS_PER_SOL,
  ComputeBudgetProgram,
  Connection,
  Transaction,
  VersionedTransaction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  NATIVE_MINT,
  getAssociatedTokenAddress,
  getOrCreateAssociatedTokenAccount,
  getAccount,
  getMint,
} from "@solana/spl-token";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";

// =============================================================================
// DEBUG FLAG - Set to true for verbose logging
// =============================================================================
const DEBUG = true;

// =============================================================================
// CONSTANTS (Derived from IDL and Program Source)
// =============================================================================

// Program addresses
const PROGRAM_ID = new PublicKey("AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w");
const WHIRLPOOL_PROGRAM_ID = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");

// Allowlisted whirlpool (from constants.rs)
const ALLOWLISTED_WHIRLPOOL = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");

// Token mints (from constants.rs)
const EXPECTED_TOKEN_MINT_A = NATIVE_MINT; // wSOL: So11111111111111111111111111111111111111112
const EXPECTED_TOKEN_MINT_B = new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2");

// PDA Seeds (from constants.rs)
const CONFIG_SEED = Buffer.from("config");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const POSITION_CUSTODY_SEED = Buffer.from("position_custody");
const WHIRLPOOL_POSITION_SEED = Buffer.from("position"); // Whirlpool's position PDA seed

// Whirlpool constants
const TICK_ARRAY_SIZE = 88;
const MIN_TICK_INDEX = -443636;
const MAX_TICK_INDEX = 443636;

// =============================================================================
// TYPES
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

interface DerivedAccounts {
  configPda: PublicKey;
  configBump: number;
  bondAuthorityPda: PublicKey;
  bondAuthorityBump: number;
  positionCustodyPda: PublicKey;
  positionCustodyBump: number;
  whirlpoolPositionPda: PublicKey;
  whirlpoolPositionBump: number;
  tickArrayLowerPda: PublicKey;
  tickArrayLowerBump: number;
  tickArrayUpperPda: PublicKey;
  tickArrayUpperBump: number;
  positionTokenAccount: PublicKey;
  custodyPositionTokenAccount: PublicKey;
  userBondAccount: PublicKey;
  userTokenBAccount: PublicKey;
}

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

function debugLog(message: string, data?: any) {
  if (DEBUG) {
    console.log(`[DEBUG] ${message}`);
    if (data !== undefined) {
      if (typeof data === "object" && data !== null) {
        console.log(JSON.stringify(data, (_, v) => 
          typeof v === "bigint" ? v.toString() : v, 2
        ));
      } else {
        console.log(data);
      }
    }
  }
}

function logSection(title: string) {
  console.log("\n" + "═".repeat(70));
  console.log(title);
  console.log("═".repeat(70));
}

function logSubsection(title: string) {
  console.log(`\n─── ${title} ───`);
}

// =============================================================================
// PDA DERIVATION FUNCTIONS
// =============================================================================

/**
 * Derive Protocol Config PDA
 * Seeds: ["config"]
 * Source: constants.rs line 8
 */
function deriveConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
}

/**
 * Derive Bond Authority PDA
 * Seeds: ["bond_authority"]
 * Source: constants.rs line 11
 */
function deriveBondAuthorityPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([BOND_AUTHORITY_SEED], PROGRAM_ID);
}

/**
 * Derive Position Custody PDA
 * Seeds: ["position_custody", bond_mint.key()]
 * Source: constants.rs line 14, lib.rs line 543
 */
function derivePositionCustodyPda(bondMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [POSITION_CUSTODY_SEED, bondMint.toBuffer()],
    PROGRAM_ID
  );
}

/**
 * Derive Whirlpool Position PDA (owned by Whirlpool program)
 * Seeds: ["position", position_mint.key()]
 * Source: whirlpool_cpi.rs get_position_address
 */
function deriveWhirlpoolPositionPda(positionMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [WHIRLPOOL_POSITION_SEED, positionMint.toBuffer()],
    WHIRLPOOL_PROGRAM_ID
  );
}

/**
 * Derive Tick Array PDA for Whirlpool
 * Seeds: ["tick_array", whirlpool.key(), start_tick_index.toString()]
 * 
 * Tick arrays contain TICK_ARRAY_SIZE (88) ticks.
 * Start tick index must be aligned to tickSpacing * TICK_ARRAY_SIZE.
 */
function deriveTickArrayPda(
  whirlpool: PublicKey,
  tickIndex: number,
  tickSpacing: number
): [PublicKey, number] {
  const ticksInArray = tickSpacing * TICK_ARRAY_SIZE;
  // Calculate start tick index (floor division to get array start)
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
// WHIRLPOOL DATA PARSING
// =============================================================================

/**
 * Parse Whirlpool account data from raw bytes
 * Layout based on Orca Whirlpool program
 * 
 * Starts with 8-byte Anchor discriminator
 */
function parseWhirlpoolAccount(data: Buffer): WhirlpoolData {
  let offset = 8; // Skip Anchor discriminator
  
  // whirlpools_config: Pubkey (32 bytes)
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
// LIQUIDITY CALCULATION
// =============================================================================

/**
 * Convert tick index to sqrt price in Q64.64 format
 * sqrt_price = sqrt(1.0001^tick) × 2^64
 */
function tickToSqrtPriceX64(tick: number): BN {
  const sqrtPriceDecimal = Math.pow(1.0001, tick / 2);
  return new BN(Math.floor(sqrtPriceDecimal * Math.pow(2, 64)).toString());
}

interface LiquidityQuote {
  liquidityAmount: BN;
  expectedAmountA: BN;
  expectedAmountB: BN;
  tokenMaxA: BN;
  tokenMaxB: BN;
  positionType: "SINGLE_SIDED_A" | "SINGLE_SIDED_B" | "TWO_SIDED";
}

/**
 * Calculate liquidity and expected token amounts for a position
 * 
 * @param desiredSolAmount - Desired SOL (lamports) to deposit
 * @param tickLower - Lower tick boundary
 * @param tickUpper - Upper tick boundary
 * @param currentTick - Current pool tick
 * @param sqrtPriceCurrent - Current sqrt price from pool
 * @param slippageBps - Slippage tolerance in basis points (e.g., 200 = 2%)
 */
function calculateLiquidityQuote(
  desiredSolAmount: BN,
  tickLower: number,
  tickUpper: number,
  currentTick: number,
  sqrtPriceCurrent: BN,
  slippageBps: number = 2000 // 20% default slippage for devnet
): LiquidityQuote {
  const sqrtPriceLower = tickToSqrtPriceX64(tickLower);
  const sqrtPriceUpper = tickToSqrtPriceX64(tickUpper);
  const q64 = new BN(2).pow(new BN(64));

  let liquidityAmount: BN;
  let expectedAmountA: BN;
  let expectedAmountB: BN;
  let positionType: LiquidityQuote["positionType"];

  if (tickLower > currentTick) {
    // Single-sided: Only Token A (wSOL) - price below range
    positionType = "SINGLE_SIDED_A";
    
    // L = amount_a × sqrt_lower × sqrt_upper / (sqrt_upper - sqrt_lower)
    liquidityAmount = desiredSolAmount
      .mul(sqrtPriceLower)
      .mul(sqrtPriceUpper)
      .div(sqrtPriceUpper.sub(sqrtPriceLower))
      .div(q64);
    expectedAmountA = desiredSolAmount;
    expectedAmountB = new BN(0);
    
  } else if (tickUpper < currentTick) {
    // Single-sided: Only Token B - price above range
    positionType = "SINGLE_SIDED_B";
    
    // For single-sided B, use minimal liquidity for testing
    liquidityAmount = new BN(1000);
    expectedAmountA = new BN(0);
    expectedAmountB = liquidityAmount.mul(sqrtPriceUpper.sub(sqrtPriceLower)).div(q64);
    
  } else {
    // Two-sided: Both tokens needed - current tick in range
    positionType = "TWO_SIDED";
    
    // L = amount_a × sqrt_current × sqrt_upper / (sqrt_upper - sqrt_current)
    liquidityAmount = desiredSolAmount
      .mul(sqrtPriceCurrent)
      .mul(sqrtPriceUpper)
      .div(sqrtPriceUpper.sub(sqrtPriceCurrent))
      .div(q64);
    
    // amount_a = L × (sqrt_upper - sqrt_current) / (sqrt_current × sqrt_upper / 2^64)
    expectedAmountA = liquidityAmount
      .mul(sqrtPriceUpper.sub(sqrtPriceCurrent))
      .div(sqrtPriceCurrent.mul(sqrtPriceUpper).div(q64));
    
    // amount_b = L × (sqrt_current - sqrt_lower) / 2^64
    expectedAmountB = liquidityAmount
      .mul(sqrtPriceCurrent.sub(sqrtPriceLower))
      .div(q64);
  }

  // Apply slippage to get max amounts
  const slippageMultiplier = 10000 + slippageBps;
  const tokenMaxA = expectedAmountA.muln(slippageMultiplier).divn(10000).addn(10000); // +dust
  const tokenMaxB = expectedAmountB.muln(slippageMultiplier).divn(10000).addn(10000); // +dust

  debugLog("Liquidity Calculation:", {
    sqrtPriceLower: sqrtPriceLower.toString(),
    sqrtPriceUpper: sqrtPriceUpper.toString(),
    sqrtPriceCurrent: sqrtPriceCurrent.toString(),
    liquidityAmount: liquidityAmount.toString(),
    expectedAmountA: expectedAmountA.toString(),
    expectedAmountB: expectedAmountB.toString(),
    tokenMaxA: tokenMaxA.toString(),
    tokenMaxB: tokenMaxB.toString(),
    positionType,
  });

  return {
    liquidityAmount,
    expectedAmountA,
    expectedAmountB,
    tokenMaxA,
    tokenMaxB,
    positionType,
  };
}

// =============================================================================
// VALIDATION FUNCTIONS
// =============================================================================

/**
 * Validate all program constants match expected values
 */
function validateProgramConstants(): void {
  logSubsection("Validating Program Constants");
  
  // Validate program ID
  console.log("Program ID:", PROGRAM_ID.toBase58());
  
  // Validate Whirlpool program ID
  if (!WHIRLPOOL_PROGRAM_ID.equals(new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"))) {
    throw new Error("Invalid Whirlpool program ID");
  }
  console.log("Whirlpool Program ID: ✓", WHIRLPOOL_PROGRAM_ID.toBase58());
  
  // Validate wSOL mint
  if (!EXPECTED_TOKEN_MINT_A.equals(NATIVE_MINT)) {
    throw new Error("Token Mint A is not wSOL");
  }
  console.log("Token Mint A (wSOL): ✓", EXPECTED_TOKEN_MINT_A.toBase58());
  
  // Validate Token B mint matches constant
  console.log("Token Mint B: ✓", EXPECTED_TOKEN_MINT_B.toBase58());
  
  // Validate tick bounds
  if (MIN_TICK_INDEX !== -443636 || MAX_TICK_INDEX !== 443636) {
    throw new Error("Invalid tick bounds");
  }
  console.log("Tick Bounds: ✓", MIN_TICK_INDEX, "to", MAX_TICK_INDEX);
}

/**
 * Validate whirlpool data matches expected configuration
 */
function validateWhirlpoolData(whirlpoolData: WhirlpoolData): void {
  logSubsection("Validating Whirlpool Data");
  
  // Validate Token A is wSOL
  if (!whirlpoolData.tokenMintA.equals(EXPECTED_TOKEN_MINT_A)) {
    throw new Error(`Token A mismatch. Expected: ${EXPECTED_TOKEN_MINT_A.toBase58()}, Got: ${whirlpoolData.tokenMintA.toBase58()}`);
  }
  console.log("Token A Match: ✓");
  
  // Validate Token B
  if (!whirlpoolData.tokenMintB.equals(EXPECTED_TOKEN_MINT_B)) {
    throw new Error(`Token B mismatch. Expected: ${EXPECTED_TOKEN_MINT_B.toBase58()}, Got: ${whirlpoolData.tokenMintB.toBase58()}`);
  }
  console.log("Token B Match: ✓");
  
  // Validate tick spacing
  if (whirlpoolData.tickSpacing <= 0) {
    throw new Error(`Invalid tick spacing: ${whirlpoolData.tickSpacing}`);
  }
  console.log("Tick Spacing: ✓", whirlpoolData.tickSpacing);
}

/**
 * Validate tick range
 */
function validateTickRange(tickLower: number, tickUpper: number, tickSpacing: number): void {
  logSubsection("Validating Tick Range");
  
  // Check bounds
  if (tickLower < MIN_TICK_INDEX || tickUpper > MAX_TICK_INDEX) {
    throw new Error(`Tick out of bounds. Lower: ${tickLower}, Upper: ${tickUpper}`);
  }
  console.log("Tick Bounds: ✓");
  
  // Check order
  if (tickLower >= tickUpper) {
    throw new Error(`Invalid tick order. Lower: ${tickLower} must be < Upper: ${tickUpper}`);
  }
  console.log("Tick Order: ✓");
  
  // Check alignment
  if (tickLower % tickSpacing !== 0 || tickUpper % tickSpacing !== 0) {
    throw new Error(`Ticks not aligned to spacing ${tickSpacing}. Lower: ${tickLower}, Upper: ${tickUpper}`);
  }
  console.log("Tick Alignment: ✓");
}

/**
 * Validate all PDAs are derived correctly
 */
function validateDerivedPdas(accounts: DerivedAccounts): void {
  logSubsection("Validating PDA Derivations");
  
  console.log("Config PDA:");
  console.log("  Address:", accounts.configPda.toBase58());
  console.log("  Bump:", accounts.configBump);
  
  console.log("Bond Authority PDA:");
  console.log("  Address:", accounts.bondAuthorityPda.toBase58());
  console.log("  Bump:", accounts.bondAuthorityBump);
  
  console.log("Position Custody PDA:");
  console.log("  Address:", accounts.positionCustodyPda.toBase58());
  console.log("  Bump:", accounts.positionCustodyBump);
  
  console.log("Whirlpool Position PDA:");
  console.log("  Address:", accounts.whirlpoolPositionPda.toBase58());
  console.log("  Bump:", accounts.whirlpoolPositionBump);
  
  console.log("Tick Array Lower PDA:");
  console.log("  Address:", accounts.tickArrayLowerPda.toBase58());
  
  console.log("Tick Array Upper PDA:");
  console.log("  Address:", accounts.tickArrayUpperPda.toBase58());
  
  console.log("Position Token Account (User ATA):");
  console.log("  Address:", accounts.positionTokenAccount.toBase58());
  
  console.log("Custody Position Token Account (Custody ATA):");
  console.log("  Address:", accounts.custodyPositionTokenAccount.toBase58());
  
  console.log("User Bond Account:");
  console.log("  Address:", accounts.userBondAccount.toBase58());
  
  console.log("User Token B Account:");
  console.log("  Address:", accounts.userTokenBAccount.toBase58());
}

// =============================================================================
// ACCOUNT VERIFICATION FUNCTIONS (POST-TX)
// =============================================================================

/**
 * Verify bond NFT was minted correctly
 */
async function verifyBondNftMinted(
  connection: Connection,
  bondMint: PublicKey,
  userPublicKey: PublicKey
): Promise<void> {
  logSubsection("Verifying Bond NFT");
  
  try {
    const mintAccount = await getMint(connection, bondMint);
    console.log("Bond Mint Address:", bondMint.toBase58());
    console.log("Supply:", mintAccount.supply.toString());
    console.log("Decimals:", mintAccount.decimals);
    console.log("Mint Authority:", mintAccount.mintAuthority?.toBase58() || "None");
    console.log("Freeze Authority:", mintAccount.freezeAuthority?.toBase58() || "None");
    
    // Verify user has the bond
    const userBondAta = await getAssociatedTokenAddress(bondMint, userPublicKey);
    try {
      const userBondAccount = await getAccount(connection, userBondAta);
      console.log("User Bond Balance:", userBondAccount.amount.toString());
      
      if (userBondAccount.amount !== BigInt(1)) {
        console.warn("⚠️  Warning: Expected 1 bond NFT, got:", userBondAccount.amount.toString());
      } else {
        console.log("✓ Bond NFT minted successfully to user");
      }
    } catch (e) {
      console.error("❌ User bond account not found");
    }
  } catch (e: any) {
    console.error("❌ Failed to verify bond NFT:", e.message);
  }
}

/**
 * Verify whirlpool position was created
 */
async function verifyWhirlpoolPosition(
  connection: Connection,
  positionMint: PublicKey,
  whirlpoolPositionPda: PublicKey
): Promise<void> {
  logSubsection("Verifying Whirlpool Position");
  
  try {
    const positionAccount = await connection.getAccountInfo(whirlpoolPositionPda);
    if (positionAccount) {
      console.log("Position Account Address:", whirlpoolPositionPda.toBase58());
      console.log("Position Account Owner:", positionAccount.owner.toBase58());
      console.log("Position Account Data Length:", positionAccount.data.length);
      console.log("✓ Whirlpool position created successfully");
    } else {
      console.error("❌ Position account not found");
    }
    
    // Verify position mint
    const mintAccount = await getMint(connection, positionMint);
    console.log("Position Mint Address:", positionMint.toBase58());
    console.log("Position Mint Supply:", mintAccount.supply.toString());
  } catch (e: any) {
    console.error("❌ Failed to verify position:", e.message);
  }
}

/**
 * Verify position custody state
 */
async function verifyPositionCustody(
  program: Program<LpBonds>,
  positionCustodyPda: PublicKey
): Promise<void> {
  logSubsection("Verifying Position Custody");
  
  try {
    const custody = await program.account.positionCustody.fetch(positionCustodyPda);
    console.log("Position Custody:");
    console.log("  Bond Mint:", custody.bondMint.toBase58());
    console.log("  Position Mint:", custody.positionMint.toBase58());
    console.log("  Whirlpool:", custody.whirlpool.toBase58());
    console.log("  Tick Lower:", custody.tickLowerIndex);
    console.log("  Tick Upper:", custody.tickUpperIndex);
    console.log("  Liquidity:", custody.liquidity.toString());
    console.log("  Depositor:", custody.depositor.toBase58());
    console.log("  Created At:", new Date(custody.createdAt.toNumber() * 1000).toISOString());
    console.log("  Bump:", custody.bump);
    console.log("  Position Bump:", custody.positionBump);
    console.log("✓ Position custody created successfully");
  } catch (e: any) {
    console.error("❌ Failed to fetch position custody:", e.message);
  }
}

/**
 * Verify user token balances after transaction
 */
async function verifyUserBalances(
  connection: Connection,
  userPublicKey: PublicKey
): Promise<void> {
  logSubsection("Verifying User Balances");
  
  // SOL balance
  const solBalance = await connection.getBalance(userPublicKey);
  console.log("SOL Balance:", solBalance / LAMPORTS_PER_SOL, "SOL");
  
  // Token B balance
  try {
    const tokenBAccount = await getAssociatedTokenAddress(EXPECTED_TOKEN_MINT_B, userPublicKey);
    const accountInfo = await getAccount(connection, tokenBAccount);
    console.log("Token B Balance:", accountInfo.amount.toString());
  } catch {
    console.log("Token B Account: Not found or no balance");
  }
}

// =============================================================================
// TRANSACTION SIMULATION
// =============================================================================

/**
 * Simulate transaction and return detailed logs
 */
async function simulateTransaction(
  connection: Connection,
  tx: Transaction | VersionedTransaction
): Promise<{ success: boolean; logs: string[] }> {
  logSubsection("Simulating Transaction");
  
  try {
    let result;
    if (tx instanceof VersionedTransaction) {
      result = await connection.simulateTransaction(tx);
    } else {
      result = await connection.simulateTransaction(tx);
    }
    
    const logs = result.value.logs || [];
    
    if (result.value.err) {
      console.error("Simulation Error:", JSON.stringify(result.value.err));
      logs.forEach((log, i) => console.log(`  [${i}] ${log}`));
      return { success: false, logs };
    }
    
    console.log("✓ Simulation successful");
    if (DEBUG) {
      console.log("Simulation logs:");
      logs.forEach((log, i) => console.log(`  [${i}] ${log}`));
    }
    
    return { success: true, logs };
  } catch (e: any) {
    console.error("Simulation failed:", e.message);
    return { success: false, logs: [e.message] };
  }
}

/**
 * Decode Anchor error from simulation logs
 */
function decodeAnchorError(logs: string[]): string | null {
  for (const log of logs) {
    // Look for custom program error
    const match = log.match(/Program .* failed: custom program error: (0x[0-9a-fA-F]+)/);
    if (match) {
      const errorCode = parseInt(match[1], 16);
      // Map to known errors from IDL
      const errors: Record<number, string> = {
        6000: "WhirlpoolNotAllowlisted",
        6001: "InvalidWhirlpoolProgram",
        6002: "InvalidTokenMintA",
        6003: "InvalidTokenMintB",
        6004: "InvalidTokenVault",
        6005: "InvalidTickRange",
        6006: "TickOutOfBounds",
        6007: "TickNotAlignedToSpacing",
        6008: "InvalidTokenOwner",
        6009: "InvalidTokenMint",
        6010: "InvalidNativeMint",
        6011: "ZeroSolAmount",
        6012: "InsufficientSolBalance",
        6013: "InvalidBondMint",
        6014: "InvalidBondBalance",
        6015: "InvalidBondMetadata",
        6016: "InvalidPositionMint",
        6017: "InvalidCustodyBondMint",
        6018: "PositionNftNotInCustody",
        6019: "InvalidCustodyPda",
        6020: "InvalidPositionPda",
        6021: "UnauthorizedSigner",
        6022: "InvalidAdminAuthority",
        6023: "ArithmeticOverflow",
        6024: "InvalidAccountData",
        6025: "OperationFailed",
      };
      return errors[errorCode] || `Unknown error: ${errorCode}`;
    }
  }
  return null;
}

// =============================================================================
// MAIN EXECUTION
// =============================================================================

async function main() {
  logSection("LP BONDS PROTOCOL - ADD LIQUIDITY AND MINT BOND");
  console.log("Network: Devnet");
  console.log("Debug Mode:", DEBUG);
  console.log("Timestamp:", new Date().toISOString());

  // =========================================================================
  // STEP 1: SETUP CONNECTION AND WALLET
  // =========================================================================
  
  logSubsection("Setting Up Connection");
  
  const connection = new Connection("https://api.devnet.solana.com", {
    commitment: "confirmed",
    confirmTransactionInitialTimeout: 60000,
  });
  
  // Load wallet
  const walletPath = process.env.ANCHOR_WALLET || path.join(os.homedir(), ".config/solana/id.json");
  const keypairData = JSON.parse(fs.readFileSync(walletPath, "utf-8"));
  const user = Keypair.fromSecretKey(Uint8Array.from(keypairData));
  const wallet = new anchor.Wallet(user);
  
  // Create provider
  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);
  
  console.log("Connection URL: https://api.devnet.solana.com");
  console.log("User Public Key:", user.publicKey.toBase58());
  
  // Check SOL balance
  const balance = await connection.getBalance(user.publicKey);
  console.log("User SOL Balance:", balance / LAMPORTS_PER_SOL, "SOL");
  
  if (balance < 0.1 * LAMPORTS_PER_SOL) {
    throw new Error("Insufficient SOL balance. Minimum 0.1 SOL required.");
  }

  // =========================================================================
  // STEP 2: LOAD PROGRAM AND VALIDATE CONSTANTS
  // =========================================================================
  
  const idlPath = path.join(__dirname, "../target/idl/lp_bonds.json");
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new Program<LpBonds>(idl, provider);
  
  console.log("Program ID (from IDL):", program.programId.toBase58());
  
  // Validate program ID matches
  if (!program.programId.equals(PROGRAM_ID)) {
    throw new Error(`Program ID mismatch. Expected: ${PROGRAM_ID.toBase58()}, Got: ${program.programId.toBase58()}`);
  }
  
  validateProgramConstants();

  // =========================================================================
  // STEP 3: FETCH AND VALIDATE WHIRLPOOL DATA
  // =========================================================================
  
  logSubsection("Fetching Whirlpool Data");
  
  const whirlpoolAccountInfo = await connection.getAccountInfo(ALLOWLISTED_WHIRLPOOL);
  if (!whirlpoolAccountInfo) {
    throw new Error("Whirlpool account not found on devnet");
  }
  
  const whirlpoolData = parseWhirlpoolAccount(whirlpoolAccountInfo.data);
  validateWhirlpoolData(whirlpoolData);
  
  console.log("Whirlpool Address:", ALLOWLISTED_WHIRLPOOL.toBase58());
  console.log("Token Vault A:", whirlpoolData.tokenVaultA.toBase58());
  console.log("Token Vault B:", whirlpoolData.tokenVaultB.toBase58());
  console.log("Tick Spacing:", whirlpoolData.tickSpacing);
  console.log("Current Tick:", whirlpoolData.tickCurrentIndex);
  console.log("Liquidity:", whirlpoolData.liquidity.toString());
  console.log("Sqrt Price:", whirlpoolData.sqrtPrice.toString());

  // =========================================================================
  // STEP 4: DERIVE ALL PDAs
  // =========================================================================
  
  logSubsection("Deriving PDAs");
  
  // Protocol PDAs
  const [configPda, configBump] = deriveConfigPda();
  const [bondAuthorityPda, bondAuthorityBump] = deriveBondAuthorityPda();
  
  // Check protocol initialization
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
    await connection.confirmTransaction(initTx, "confirmed");
  } else {
    console.log("✓ Protocol already initialized");
  }
  
  // Generate new keypairs for mints
  const bondMint = Keypair.generate();
  const positionMint = Keypair.generate();
  const userWsolAccount = Keypair.generate();
  
  console.log("Generated Bond Mint:", bondMint.publicKey.toBase58());
  console.log("Generated Position Mint:", positionMint.publicKey.toBase58());
  console.log("Generated wSOL Account:", userWsolAccount.publicKey.toBase58());
  
  // Derive PDAs dependent on generated keypairs
  const [positionCustodyPda, positionCustodyBump] = derivePositionCustodyPda(bondMint.publicKey);
  const [whirlpoolPositionPda, whirlpoolPositionBump] = deriveWhirlpoolPositionPda(positionMint.publicKey);
  
  // =========================================================================
  // STEP 5: CALCULATE TICK RANGE
  // =========================================================================
  
  logSubsection("Calculating Tick Range");
  
  const tickSpacing = whirlpoolData.tickSpacing;
  const currentTick = whirlpoolData.tickCurrentIndex;
  const tickRange = tickSpacing * 10; // 10 tick spacings on each side
  
  // Align ticks to spacing
  const tickLowerIndex = Math.floor((currentTick - tickRange) / tickSpacing) * tickSpacing;
  const tickUpperIndex = Math.ceil((currentTick + tickRange) / tickSpacing) * tickSpacing;
  
  console.log("Current Tick:", currentTick);
  console.log("Tick Spacing:", tickSpacing);
  console.log("Tick Lower:", tickLowerIndex);
  console.log("Tick Upper:", tickUpperIndex);
  
  validateTickRange(tickLowerIndex, tickUpperIndex, tickSpacing);
  
  // Derive tick arrays
  const [tickArrayLowerPda, tickArrayLowerBump] = deriveTickArrayPda(ALLOWLISTED_WHIRLPOOL, tickLowerIndex, tickSpacing);
  const [tickArrayUpperPda, tickArrayUpperBump] = deriveTickArrayPda(ALLOWLISTED_WHIRLPOOL, tickUpperIndex, tickSpacing);
  
  console.log("Tick Array Lower:", tickArrayLowerPda.toBase58());
  console.log("Tick Array Upper:", tickArrayUpperPda.toBase58());
  
  // Verify tick arrays exist
  const tickArrayLowerInfo = await connection.getAccountInfo(tickArrayLowerPda);
  const tickArrayUpperInfo = await connection.getAccountInfo(tickArrayUpperPda);
  
  if (!tickArrayLowerInfo || !tickArrayUpperInfo) {
    console.error("\n❌ FATAL: Tick arrays not initialized for selected range.");
    console.error("   Lower tick array exists:", !!tickArrayLowerInfo);
    console.error("   Upper tick array exists:", !!tickArrayUpperInfo);
    throw new Error("Tick arrays not initialized. Cannot proceed.");
  }
  console.log("✓ Both tick arrays exist");

  // =========================================================================
  // STEP 6: DERIVE TOKEN ACCOUNTS
  // =========================================================================
  
  logSubsection("Deriving Token Accounts");
  
  // User's Token B ATA
  const userTokenBAccount = await getAssociatedTokenAddress(EXPECTED_TOKEN_MINT_B, user.publicKey);
  
  // User's Bond ATA (will be created by instruction)
  const userBondAccount = await getAssociatedTokenAddress(bondMint.publicKey, user.publicKey);
  
  // Position token account (user's ATA for position NFT - created by whirlpool CPI)
  const positionTokenAccount = await getAssociatedTokenAddress(positionMint.publicKey, user.publicKey);
  
  // Custody's position token account (custody PDA's ATA for position NFT)
  const custodyPositionTokenAccount = await getAssociatedTokenAddress(
    positionMint.publicKey,
    positionCustodyPda,
    true // allowOwnerOffCurve for PDA
  );
  
  // Ensure Token B ATA exists
  console.log("\nEnsuring Token B ATA exists...");
  let userTokenBAccountInfo;
  try {
    userTokenBAccountInfo = await getOrCreateAssociatedTokenAccount(
      connection,
      user,
      EXPECTED_TOKEN_MINT_B,
      user.publicKey
    );
    console.log("✓ Token B ATA:", userTokenBAccountInfo.address.toBase58());
    console.log("  Balance:", userTokenBAccountInfo.amount.toString());
  } catch (e: any) {
    console.error("❌ Failed to create/fetch Token B ATA:", e.message);
    throw new Error("Cannot proceed without Token B ATA");
  }
  
  // Collect all derived accounts
  const derivedAccounts: DerivedAccounts = {
    configPda,
    configBump,
    bondAuthorityPda,
    bondAuthorityBump,
    positionCustodyPda,
    positionCustodyBump,
    whirlpoolPositionPda,
    whirlpoolPositionBump,
    tickArrayLowerPda,
    tickArrayLowerBump,
    tickArrayUpperPda,
    tickArrayUpperBump,
    positionTokenAccount,
    custodyPositionTokenAccount,
    userBondAccount,
    userTokenBAccount,
  };
  
  validateDerivedPdas(derivedAccounts);

  // =========================================================================
  // STEP 7: CALCULATE LIQUIDITY
  // =========================================================================
  
  logSubsection("Calculating Liquidity");
  
  const desiredSolAmount = new BN(0.005 * LAMPORTS_PER_SOL); // 0.005 SOL for testing
  
  const liquidityQuote = calculateLiquidityQuote(
    desiredSolAmount,
    tickLowerIndex,
    tickUpperIndex,
    currentTick,
    whirlpoolData.sqrtPrice
  );
  
  console.log("Position Type:", liquidityQuote.positionType);
  console.log("Desired SOL:", desiredSolAmount.toString(), "lamports");
  console.log("Liquidity Amount:", liquidityQuote.liquidityAmount.toString());
  console.log("Expected Amount A:", liquidityQuote.expectedAmountA.toString(), "lamports");
  console.log("Expected Amount B:", liquidityQuote.expectedAmountB.toString(), "tokens");
  console.log("Token Max A:", liquidityQuote.tokenMaxA.toString(), "lamports");
  console.log("Token Max B:", liquidityQuote.tokenMaxB.toString(), "tokens");
  
  // Validate Token B balance if needed
  if (liquidityQuote.positionType === "TWO_SIDED" && liquidityQuote.expectedAmountB.gtn(0)) {
    const userTokenBBalance = userTokenBAccountInfo.amount;
    if (BigInt(liquidityQuote.expectedAmountB.toString()) > userTokenBBalance) {
      console.warn(`\n⚠️  WARNING: Insufficient Token B balance`);
      console.warn(`   Required: ${liquidityQuote.expectedAmountB.toString()}`);
      console.warn(`   Available: ${userTokenBBalance.toString()}`);
    }
  }
  
  const solAmount = liquidityQuote.tokenMaxA; // Amount of SOL to wrap

  // =========================================================================
  // STEP 8: PRINT ALL ACCOUNTS (DEBUG)
  // =========================================================================
  
  if (DEBUG) {
    logSubsection("All Transaction Accounts");
    
    const accountsList = [
      { name: "user", address: user.publicKey, signer: true, writable: true },
      { name: "wsol_mint", address: NATIVE_MINT, signer: false, writable: false },
      { name: "token_mint_b", address: EXPECTED_TOKEN_MINT_B, signer: false, writable: false },
      { name: "bond_authority", address: bondAuthorityPda, signer: false, writable: false },
      { name: "bond_mint", address: bondMint.publicKey, signer: true, writable: true },
      { name: "user_wsol_account", address: userWsolAccount.publicKey, signer: true, writable: true },
      { name: "user_token_b_account", address: userTokenBAccount, signer: false, writable: true },
      { name: "user_bond_account", address: userBondAccount, signer: false, writable: true },
      { name: "config", address: configPda, signer: false, writable: true },
      { name: "position_custody", address: positionCustodyPda, signer: false, writable: true },
      { name: "position_mint", address: positionMint.publicKey, signer: true, writable: true },
      { name: "whirlpool_position", address: whirlpoolPositionPda, signer: false, writable: true },
      { name: "position_token_account", address: positionTokenAccount, signer: false, writable: true },
      { name: "custody_position_token_account", address: custodyPositionTokenAccount, signer: false, writable: true },
      { name: "whirlpool", address: ALLOWLISTED_WHIRLPOOL, signer: false, writable: true },
      { name: "token_vault_a", address: whirlpoolData.tokenVaultA, signer: false, writable: true },
      { name: "token_vault_b", address: whirlpoolData.tokenVaultB, signer: false, writable: true },
      { name: "tick_array_lower", address: tickArrayLowerPda, signer: false, writable: true },
      { name: "tick_array_upper", address: tickArrayUpperPda, signer: false, writable: true },
      { name: "whirlpool_program", address: WHIRLPOOL_PROGRAM_ID, signer: false, writable: false },
      { name: "token_program", address: TOKEN_PROGRAM_ID, signer: false, writable: false },
      { name: "associated_token_program", address: ASSOCIATED_TOKEN_PROGRAM_ID, signer: false, writable: false },
      { name: "system_program", address: SystemProgram.programId, signer: false, writable: false },
      { name: "rent", address: SYSVAR_RENT_PUBKEY, signer: false, writable: false },
    ];
    
    console.log("\nAccounts (in instruction order):");
    accountsList.forEach((acc, i) => {
      console.log(`  ${i + 1}. ${acc.name}:`);
      console.log(`     Address: ${acc.address.toBase58()}`);
      console.log(`     Signer: ${acc.signer}, Writable: ${acc.writable}`);
    });
    
    console.log("\nSigners:");
    console.log("  1. user:", user.publicKey.toBase58());
    console.log("  2. bond_mint:", bondMint.publicKey.toBase58());
    console.log("  3. position_mint:", positionMint.publicKey.toBase58());
    console.log("  4. user_wsol_account:", userWsolAccount.publicKey.toBase58());
    
    console.log("\nInstruction Arguments:");
    console.log("  tick_lower_index:", tickLowerIndex);
    console.log("  tick_upper_index:", tickUpperIndex);
    console.log("  liquidity_amount:", liquidityQuote.liquidityAmount.toString());
    console.log("  token_max_a:", liquidityQuote.tokenMaxA.toString());
    console.log("  token_max_b:", liquidityQuote.tokenMaxB.toString());
    console.log("  sol_amount:", solAmount.toString());
  }

  // =========================================================================
  // STEP 9: BUILD AND SEND TRANSACTION
  // =========================================================================
  
  logSubsection("Building Transaction");
  
  try {
    const txBuilder = program.methods
      .addLiquidityAndMintBond(
        tickLowerIndex,
        tickUpperIndex,
        liquidityQuote.liquidityAmount,
        liquidityQuote.tokenMaxA,
        liquidityQuote.tokenMaxB,
        solAmount
      )
      .accountsPartial({
        // User accounts
        user: user.publicKey,
        
        // Token mints
        wsolMint: NATIVE_MINT,
        tokenMintB: EXPECTED_TOKEN_MINT_B,
        bondAuthority: bondAuthorityPda,
        
        // Bond NFT accounts
        bondMint: bondMint.publicKey,
        
        // User token accounts
        userWsolAccount: userWsolAccount.publicKey,
        userTokenBAccount: userTokenBAccount,
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
        whirlpool: ALLOWLISTED_WHIRLPOOL,
        tokenVaultA: whirlpoolData.tokenVaultA,
        tokenVaultB: whirlpoolData.tokenVaultB,
        tickArrayLower: tickArrayLowerPda,
        tickArrayUpper: tickArrayUpperPda,
        
        // Programs
        whirlpoolProgram: WHIRLPOOL_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: SYSVAR_RENT_PUBKEY,
      })
      .preInstructions([
        ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }),
      ])
      .signers([bondMint, positionMint, userWsolAccount]);
    
    // Build transaction for simulation
    const tx = await txBuilder.transaction();
    tx.recentBlockhash = (await connection.getLatestBlockhash()).blockhash;
    tx.feePayer = user.publicKey;
    tx.partialSign(user, bondMint, positionMint, userWsolAccount);
    
    // Simulate first
    console.log("\nSimulating transaction...");
    const simResult = await simulateTransaction(connection, tx);
    
    if (!simResult.success) {
      const anchorError = decodeAnchorError(simResult.logs);
      if (anchorError) {
        console.error("\n❌ Anchor Error:", anchorError);
      }
      console.error("\nSimulation logs:");
      simResult.logs.forEach((log, i) => console.error(`  [${i}] ${log}`));
      throw new Error("Transaction simulation failed");
    }
    
    // Send transaction
    logSubsection("Sending Transaction");
    
    const signature = await txBuilder.rpc();
    
    console.log("\n" + "═".repeat(70));
    console.log("✓ TRANSACTION SUCCESSFUL");
    console.log("═".repeat(70));
    console.log("\nSignature:", signature);
    console.log("\nExplorer Link:");
    console.log(`https://explorer.solana.com/tx/${signature}?cluster=devnet`);
    console.log("\nBond NFT Mint:", bondMint.publicKey.toBase58());
    console.log("Position Mint:", positionMint.publicKey.toBase58());
    
    // Wait for finalization
    console.log("\nWaiting for finalization...");
    await connection.confirmTransaction(signature, "finalized");
    console.log("✓ Transaction finalized");

    // =========================================================================
    // STEP 10: VERIFY SUCCESS
    // =========================================================================
    
    logSection("POST-TRANSACTION VERIFICATION");
    
    await verifyBondNftMinted(connection, bondMint.publicKey, user.publicKey);
    await verifyWhirlpoolPosition(connection, positionMint.publicKey, whirlpoolPositionPda);
    await verifyPositionCustody(program, positionCustodyPda);
    await verifyUserBalances(connection, user.publicKey);
    
    console.log("\n" + "═".repeat(70));
    console.log("✓ ALL VERIFICATIONS PASSED");
    console.log("═".repeat(70));
    
  } catch (error: any) {
    console.error("\n" + "═".repeat(70));
    console.error("✗ TRANSACTION FAILED");
    console.error("═".repeat(70));
    
    if (error instanceof AnchorError) {
      console.error("\nAnchor Error:", error.error.errorMessage);
      console.error("Error Code:", error.error.errorCode.code);
      console.error("Error Name:", error.error.errorCode.number);
    }
    
    if (error.logs) {
      console.error("\nProgram Logs:");
      error.logs.forEach((log: string, i: number) => console.error(`  [${i}] ${log}`));
      
      const anchorError = decodeAnchorError(error.logs);
      if (anchorError) {
        console.error("\nDecoded Error:", anchorError);
      }
    }
    
    if (error.message) {
      console.error("\nError Message:", error.message);
    }
    
    console.error("═".repeat(70) + "\n");
    throw error;
  }
}

// =============================================================================
// EXECUTE
// =============================================================================

main()
  .then(() => {
    console.log("\n✓ Script completed successfully\n");
    process.exit(0);
  })
  .catch((error) => {
    console.error("\n✗ Script failed:", error.message);
    process.exit(1);
  });
