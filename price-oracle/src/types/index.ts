/**
 * ============================================================================
 * TYPE DEFINITIONS FOR ORCA WHIRLPOOL ORACLE
 * ============================================================================
 * 
 * Core types for whirlpool data, position data, and API responses.
 * These types are designed for full-range liquidity positions only.
 */

import { PublicKey } from "@solana/web3.js";

/**
 * Whirlpool account data decoded from on-chain.
 * 
 * Key fields for price oracle:
 * - sqrtPriceX64: Current price as sqrt in Q64.64 format
 * - tickCurrentIndex: Current tick (log1.0001(price))
 * - liquidity: Active liquidity in the current tick range
 * - tokenVaultA/B: Addresses of token vaults
 * - tokenMintA/B: Token mint addresses
 */
export interface WhirlpoolData {
  address: string;
  whirlpoolsConfig: string;
  whirlpoolBump: number;
  tickSpacing: number;
  feeRate: number;
  protocolFeeRate: number;
  
  // Core price data
  sqrtPriceX64: string;
  tickCurrentIndex: number;
  liquidity: string;
  
  // Token configuration
  tokenMintA: string;
  tokenMintB: string;
  tokenVaultA: string;
  tokenVaultB: string;
  
  // Vault balances (fetched separately)
  vaultBalanceA: string;
  vaultBalanceB: string;
  
  // Fee growth
  feeGrowthGlobalA: string;
  feeGrowthGlobalB: string;
  protocolFeeOwedA: string;
  protocolFeeOwedB: string;
}

/**
 * Position account data decoded from on-chain.
 * 
 * Whirlpool positions are NFT-based:
 * - Each position has a unique position mint (NFT)
 * - Position data is stored in a PDA derived from the position mint
 * - liquidity represents the position's share of the pool
 */
export interface PositionData {
  address: string;
  whirlpool: string;
  positionMint: string;
  liquidity: string;
  tickLowerIndex: number;
  tickUpperIndex: number;
  
  // Fee tracking
  feeGrowthCheckpointA: string;
  feeGrowthCheckpointB: string;
  feeOwedA: string;
  feeOwedB: string;
  
  // Whether this is a full-range position
  isFullRange: boolean;
}

/**
 * Computed position valuation.
 * 
 * For FULL-RANGE positions, the math simplifies significantly:
 * - amountA = L / sqrtPrice
 * - amountB = L * sqrtPrice
 * 
 * This is because full-range positions span MIN_TICK to MAX_TICK,
 * making them behave like classic constant-product (x*y=k) positions.
 */
export interface PositionValuation {
  positionAddress: string;
  positionMint: string;
  
  // Raw amounts (smallest units)
  amountARaw: string;
  amountBRaw: string;
  
  // Human-readable amounts (adjusted for decimals)
  amountA: string;
  amountB: string;
  
  // Liquidity share
  liquidity: string;
  liquidityShare: string; // Percentage of total pool liquidity
  
  // Value in token B terms (using current price)
  totalValueInB: string;
  
  // Tick range verification
  tickLowerIndex: number;
  tickUpperIndex: number;
  isFullRange: boolean;
  
  // Fees owed
  feeOwedA: string;
  feeOwedB: string;
}

/**
 * Price data computed from sqrtPriceX64.
 * 
 * MATH EXPLANATION:
 * ─────────────────
 * sqrtPriceX64 is stored as a Q64.64 fixed-point number:
 * - 64 bits for integer part
 * - 64 bits for fractional part
 * 
 * To get the actual sqrtPrice:
 *   sqrtPrice = sqrtPriceX64 / 2^64
 * 
 * To get the price:
 *   price = sqrtPrice^2 = (sqrtPriceX64 / 2^64)^2
 * 
 * The price represents: how many tokenB per tokenA
 * 
 * To get human-readable price (adjusted for decimals):
 *   adjustedPrice = price * (10^decimalsA / 10^decimalsB)
 */
export interface PriceData {
  // Raw price (tokenB per tokenA in smallest units)
  priceRaw: string;
  
  // Human-readable price (adjusted for decimals)
  price: string;
  
  // Inverse price (tokenA per tokenB)
  inversePriceRaw: string;
  inversePrice: string;
  
  // sqrtPrice components for debugging
  sqrtPriceX64: string;
  sqrtPrice: string;
  
  // Current tick
  tickCurrentIndex: number;
}

/**
 * Aggregated oracle response.
 * 
 * Provides complete view of:
 * - Whirlpool state
 * - Current price
 * - All positions with valuations
 * - Aggregated totals
 */
export interface OracleResponse {
  // Timestamp
  timestamp: number;
  
  // Whirlpool data
  whirlpool: WhirlpoolData;
  
  // Price data
  price: PriceData;
  
  // Token metadata
  tokenA: {
    mint: string;
    decimals: number;
    symbol?: string;
  };
  tokenB: {
    mint: string;
    decimals: number;
    symbol?: string;
  };
  
  // All positions
  positions: PositionValuation[];
  
  // Aggregated metrics
  aggregate: {
    // Total positions count
    totalPositions: number;
    
    // Full-range positions count
    fullRangePositions: number;
    
    // Total liquidity across all positions
    totalLiquidity: string;
    
    // Total token exposure
    totalAmountA: string;
    totalAmountB: string;
    totalAmountARaw: string;
    totalAmountBRaw: string;
    
    // TVL in token B terms
    tvlInB: string;
    
    // TVL in token A terms
    tvlInA: string;
    
    // Liquidity distribution
    liquidityDistribution: Array<{
      positionAddress: string;
      share: string;
    }>;
  };
}

/**
 * Whirlpool account layout constants.
 * 
 * The Whirlpool account uses Anchor discriminator (8 bytes) followed by:
 * - whirlpoolsConfig: Pubkey (32 bytes)
 * - whirlpoolBump: [u8; 1] (1 byte)
 * - tickSpacing: u16 (2 bytes)
 * - tickSpacingSeed: [u8; 2] (2 bytes)
 * - feeRate: u16 (2 bytes)
 * - protocolFeeRate: u16 (2 bytes)
 * - liquidity: u128 (16 bytes)
 * - sqrtPrice: u128 (16 bytes)
 * - tickCurrentIndex: i32 (4 bytes)
 * - ... etc
 */
export const WHIRLPOOL_ACCOUNT_SIZE = 653;

/**
 * Position account layout constants.
 * Anchor discriminator (8 bytes) + Position data
 */
export const POSITION_ACCOUNT_SIZE = 216;

/**
 * Tick index bounds for Orca Whirlpool.
 * 
 * TICK MATH EXPLANATION:
 * ──────────────────────
 * Each tick represents a price point: price = 1.0001^tick
 * 
 * MIN_TICK_INDEX = -443636 → price ≈ 0 (essentially zero)
 * MAX_TICK_INDEX = 443636  → price ≈ ∞ (very large number)
 * 
 * Full-range positions set:
 * - tickLowerIndex = MIN_TICK_INDEX (aligned to tick spacing)
 * - tickUpperIndex = MAX_TICK_INDEX (aligned to tick spacing)
 * 
 * This spans the entire possible price range, making the position
 * behave like a classic constant-product AMM (Uniswap v2 style).
 */
export const MIN_TICK_INDEX = -443636;
export const MAX_TICK_INDEX = 443636;

/**
 * Orca Whirlpool Program ID (same on mainnet and devnet)
 */
export const WHIRLPOOL_PROGRAM_ID = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
