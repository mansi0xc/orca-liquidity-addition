/**
 * ============================================================================
 * WHIRLPOOL MATH UTILITIES
 * ============================================================================
 * 
 * Precise mathematical operations for Orca Whirlpool price and liquidity
 * calculations. All core operations use BigInt for precision.
 * 
 * KEY CONCEPTS:
 * ─────────────
 * 
 * 1. Q64.64 FIXED-POINT FORMAT
 *    Whirlpool stores sqrtPrice as a Q64.64 fixed-point number:
 *    - 64 bits for the integer part
 *    - 64 bits for the fractional part
 *    - Value = sqrtPriceX64 / 2^64
 *    
 *    This format allows representing very small and very large numbers
 *    with high precision without floating-point errors.
 * 
 * 2. TICK INDEX AND PRICE RELATIONSHIP
 *    price = 1.0001^tick
 *    tick = log(price) / log(1.0001)
 *    
 *    This logarithmic spacing means:
 *    - Each tick represents a 0.01% price change
 *    - Tick 0 = price of 1
 *    - Positive ticks = price > 1
 *    - Negative ticks = price < 1
 * 
 * 3. WHY FULL-RANGE SIMPLIFIES MATH
 *    For concentrated liquidity (Uniswap v3 / Orca Whirlpool):
 *    
 *    General formulas:
 *      amountA = L * (1/sqrtP - 1/sqrtPu)  [when P < Pu]
 *      amountB = L * (sqrtP - sqrtPl)      [when P > Pl]
 *    
 *    For FULL-RANGE (Pl → 0, Pu → ∞):
 *      sqrtPl → 0, sqrtPu → ∞
 *      1/sqrtPu → 0, sqrtPl → 0
 *      
 *    Simplified:
 *      amountA = L / sqrtP
 *      amountB = L * sqrtP
 *    
 *    This is equivalent to constant-product AMM (x * y = k):
 *      L^2 = x * y (where L is liquidity)
 *      amountA * amountB = L^2
 * 
 * 4. WHY FULL-RANGE BEHAVES LIKE CONSTANT PRODUCT
 *    In a constant-product AMM:
 *      x * y = k
 *      price = y / x
 *    
 *    For full-range:
 *      L^2 = amountA * amountB
 *      amountA = L / sqrtP
 *      amountB = L * sqrtP
 *      amountA * amountB = (L / sqrtP) * (L * sqrtP) = L^2 ✓
 *    
 *    The position is always "in range" because the range is infinite.
 *    No impermanent loss calculation needed - the position simply
 *    follows the price, always providing liquidity.
 */

import BigNumber from "bignumber.js";

// Configure BigNumber for maximum precision
BigNumber.config({
  DECIMAL_PLACES: 40,
  ROUNDING_MODE: BigNumber.ROUND_DOWN,
  EXPONENTIAL_AT: [-40, 40],
});

/**
 * 2^64 as BigInt - fundamental constant for Q64.64 math
 */
export const Q64: bigint = 1n << 64n;

/**
 * 2^128 as BigInt - used for intermediate calculations
 */
export const Q128: bigint = 1n << 128n;

/**
 * Minimum tick index for Orca Whirlpool
 * At this tick: price ≈ 1.0001^(-443636) ≈ 2.9e-19
 */
export const MIN_TICK_INDEX = -443636;

/**
 * Maximum tick index for Orca Whirlpool
 * At this tick: price ≈ 1.0001^(443636) ≈ 3.4e18
 */
export const MAX_TICK_INDEX = 443636;

/**
 * Convert sqrtPriceX64 to actual sqrtPrice.
 * 
 * sqrtPrice = sqrtPriceX64 / 2^64
 * 
 * @param sqrtPriceX64 - sqrt price in Q64.64 format (as bigint or string)
 * @returns sqrtPrice as BigNumber
 */
export function sqrtPriceX64ToSqrtPrice(sqrtPriceX64: bigint | string): BigNumber {
  const sqrtPriceX64Bn = new BigNumber(sqrtPriceX64.toString());
  const q64 = new BigNumber(Q64.toString());
  return sqrtPriceX64Bn.dividedBy(q64);
}

/**
 * Convert sqrtPriceX64 to price.
 * 
 * price = (sqrtPriceX64 / 2^64)^2
 * 
 * The price represents: how many tokenB per 1 tokenA (in raw units)
 * 
 * @param sqrtPriceX64 - sqrt price in Q64.64 format
 * @returns price as BigNumber (tokenB per tokenA in smallest units)
 */
export function sqrtPriceX64ToPrice(sqrtPriceX64: bigint | string): BigNumber {
  const sqrtPrice = sqrtPriceX64ToSqrtPrice(sqrtPriceX64);
  return sqrtPrice.multipliedBy(sqrtPrice);
}

/**
 * Adjust price for token decimals to get human-readable price.
 * 
 * adjustedPrice = price * (10^decimalsA / 10^decimalsB)
 * 
 * Example:
 *   If tokenA has 9 decimals and tokenB has 6 decimals:
 *   adjustedPrice = price * (10^9 / 10^6) = price * 1000
 * 
 * @param priceRaw - Raw price (tokenB per tokenA in smallest units)
 * @param decimalsA - Decimals for token A
 * @param decimalsB - Decimals for token B
 * @returns Human-readable price (adjusted for decimals)
 */
export function adjustPriceForDecimals(
  priceRaw: BigNumber,
  decimalsA: number,
  decimalsB: number
): BigNumber {
  const decimalAdjustment = new BigNumber(10).pow(decimalsA - decimalsB);
  return priceRaw.multipliedBy(decimalAdjustment);
}

/**
 * Calculate token amounts for a FULL-RANGE position.
 * 
 * MATH:
 *   amountA = L / sqrtPrice
 *   amountB = L * sqrtPrice
 * 
 * Where L is the position's liquidity value.
 * 
 * This formula works because:
 * 1. Full-range spans from sqrtPl ≈ 0 to sqrtPu ≈ ∞
 * 2. The position is always "in range"
 * 3. It behaves like constant-product: amountA * amountB = L^2
 * 
 * PRECISION:
 * We use BigInt arithmetic with proper scaling to avoid precision loss.
 * 
 * @param liquidity - Position liquidity (as bigint or string)
 * @param sqrtPriceX64 - Current sqrt price in Q64.64 format
 * @returns Object with amountA and amountB as BigNumber (raw units)
 */
export function calculateFullRangeAmounts(
  liquidity: bigint | string,
  sqrtPriceX64: bigint | string
): { amountA: BigNumber; amountB: BigNumber } {
  const L = new BigNumber(liquidity.toString());
  const sqrtPrice = sqrtPriceX64ToSqrtPrice(sqrtPriceX64);
  
  // amountA = L / sqrtPrice
  const amountA = L.dividedBy(sqrtPrice);
  
  // amountB = L * sqrtPrice
  const amountB = L.multipliedBy(sqrtPrice);
  
  return { amountA, amountB };
}

/**
 * Calculate token amounts using BigInt for maximum precision.
 * 
 * For full-range positions:
 *   amountA = L * 2^64 / sqrtPriceX64
 *   amountB = L * sqrtPriceX64 / 2^64
 * 
 * @param liquidity - Position liquidity
 * @param sqrtPriceX64 - Current sqrt price in Q64.64 format
 * @returns Object with amountA and amountB as bigint (raw units, truncated)
 */
export function calculateFullRangeAmountsBigInt(
  liquidity: bigint,
  sqrtPriceX64: bigint
): { amountA: bigint; amountB: bigint } {
  // amountA = L / sqrtPrice = L * Q64 / sqrtPriceX64
  // We multiply by Q64 first to maintain precision
  const amountA = (liquidity * Q64) / sqrtPriceX64;
  
  // amountB = L * sqrtPrice = L * sqrtPriceX64 / Q64
  const amountB = (liquidity * sqrtPriceX64) / Q64;
  
  return { amountA, amountB };
}

/**
 * Convert raw token amount to human-readable format.
 * 
 * @param amountRaw - Amount in smallest units
 * @param decimals - Token decimals
 * @returns Human-readable amount as string
 */
export function formatTokenAmount(
  amountRaw: BigNumber | bigint | string,
  decimals: number
): string {
  const amount = new BigNumber(amountRaw.toString());
  const divisor = new BigNumber(10).pow(decimals);
  return amount.dividedBy(divisor).toFixed(decimals);
}

/**
 * Calculate liquidity share percentage.
 * 
 * @param positionLiquidity - Liquidity of the position
 * @param totalLiquidity - Total liquidity in pool or across positions
 * @returns Share as percentage string (e.g., "12.34")
 */
export function calculateLiquidityShare(
  positionLiquidity: bigint | string,
  totalLiquidity: bigint | string
): string {
  if (totalLiquidity.toString() === "0") return "0";
  
  const position = new BigNumber(positionLiquidity.toString());
  const total = new BigNumber(totalLiquidity.toString());
  
  return position
    .dividedBy(total)
    .multipliedBy(100)
    .toFixed(4);
}

/**
 * Calculate total value in terms of one token.
 * 
 * For valuation in tokenB terms:
 *   totalValue = amountB + (amountA * price)
 * 
 * @param amountA - Amount of token A
 * @param amountB - Amount of token B
 * @param priceAInB - Price of A in terms of B
 * @returns Total value in tokenB units
 */
export function calculateTotalValueInB(
  amountA: BigNumber,
  amountB: BigNumber,
  priceAInB: BigNumber
): BigNumber {
  return amountB.plus(amountA.multipliedBy(priceAInB));
}

/**
 * Calculate total value in terms of token A.
 * 
 * @param amountA - Amount of token A
 * @param amountB - Amount of token B
 * @param priceBInA - Price of B in terms of A (inverse price)
 * @returns Total value in tokenA units
 */
export function calculateTotalValueInA(
  amountA: BigNumber,
  amountB: BigNumber,
  priceBInA: BigNumber
): BigNumber {
  return amountA.plus(amountB.multipliedBy(priceBInA));
}

/**
 * Check if tick bounds represent a full-range position.
 * 
 * Full-range means:
 * - tickLowerIndex is at or near MIN_TICK_INDEX (aligned to tick spacing)
 * - tickUpperIndex is at or near MAX_TICK_INDEX (aligned to tick spacing)
 * 
 * @param tickLowerIndex - Lower tick bound
 * @param tickUpperIndex - Upper tick bound
 * @param tickSpacing - Whirlpool tick spacing
 * @returns true if this is a full-range position
 */
export function isFullRangePosition(
  tickLowerIndex: number,
  tickUpperIndex: number,
  tickSpacing: number
): boolean {
  // Calculate aligned min/max ticks
  const alignedMinTick = Math.ceil(MIN_TICK_INDEX / tickSpacing) * tickSpacing;
  const alignedMaxTick = Math.floor(MAX_TICK_INDEX / tickSpacing) * tickSpacing;
  
  return tickLowerIndex === alignedMinTick && tickUpperIndex === alignedMaxTick;
}

/**
 * Get aligned tick bounds for full-range position.
 * 
 * @param tickSpacing - Whirlpool tick spacing
 * @returns Object with aligned tickLower and tickUpper
 */
export function getFullRangeTickBounds(tickSpacing: number): {
  tickLower: number;
  tickUpper: number;
} {
  const tickLower = Math.ceil(MIN_TICK_INDEX / tickSpacing) * tickSpacing;
  const tickUpper = Math.floor(MAX_TICK_INDEX / tickSpacing) * tickSpacing;
  
  return { tickLower, tickUpper };
}

/**
 * Convert tick to price.
 * 
 * price = 1.0001^tick
 * 
 * @param tick - Tick index
 * @returns Price as BigNumber
 */
export function tickToPrice(tick: number): BigNumber {
  // price = 1.0001^tick
  const base = new BigNumber("1.0001");
  return base.pow(tick);
}

/**
 * Parse a 128-bit unsigned integer from a buffer.
 * 
 * @param buffer - Buffer containing the u128
 * @param offset - Offset in buffer
 * @returns u128 as bigint
 */
export function readU128(buffer: Buffer, offset: number): bigint {
  // Read as little-endian (Solana is little-endian)
  const low = buffer.readBigUInt64LE(offset);
  const high = buffer.readBigUInt64LE(offset + 8);
  return (high << 64n) | low;
}

/**
 * Parse a 64-bit unsigned integer from a buffer.
 * 
 * @param buffer - Buffer containing the u64
 * @param offset - Offset in buffer
 * @returns u64 as bigint
 */
export function readU64(buffer: Buffer, offset: number): bigint {
  return buffer.readBigUInt64LE(offset);
}

/**
 * Parse a 32-bit signed integer from a buffer.
 * 
 * @param buffer - Buffer containing the i32
 * @param offset - Offset in buffer
 * @returns i32 as number
 */
export function readI32(buffer: Buffer, offset: number): number {
  return buffer.readInt32LE(offset);
}

/**
 * Parse a 16-bit unsigned integer from a buffer.
 * 
 * @param buffer - Buffer containing the u16
 * @param offset - Offset in buffer
 * @returns u16 as number
 */
export function readU16(buffer: Buffer, offset: number): number {
  return buffer.readUInt16LE(offset);
}
