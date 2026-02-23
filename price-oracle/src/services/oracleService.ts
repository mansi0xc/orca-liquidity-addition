/**
 * ============================================================================
 * ORACLE SERVICE
 * ============================================================================
 * 
 * Main service that orchestrates fetching, decoding, and computing
 * valuations for Whirlpool positions.
 * 
 * ORCHESTRATION FLOW:
 * ───────────────────
 * 1. Fetch Whirlpool account → sqrtPriceX64, liquidity, vaults, mints
 * 2. Fetch token decimals for human-readable formatting
 * 3. Fetch all Position accounts for the whirlpool
 * 4. For each position:
 *    - Verify full-range bounds
 *    - Compute token amounts using full-range formulas
 *    - Calculate USD/value in base token
 * 5. Aggregate totals: TVL, total liquidity, distributions
 * 
 * MATH RECAP (FULL RANGE):
 * ────────────────────────
 * For a full-range position with liquidity L at sqrtPrice P:
 *   amountA = L / P
 *   amountB = L * P
 * 
 * This is because full-range positions (tickLower = MIN, tickUpper = MAX)
 * are always "in range" and behave like constant-product AMM positions.
 */

import { Connection } from "@solana/web3.js";
import BigNumber from "bignumber.js";
import { WhirlpoolService } from "./whirlpoolService";
import { PositionService } from "./positionService";
import {
  OracleResponse,
  PositionValuation,
  PriceData,
  WhirlpoolData,
  PositionData,
} from "../types";
import {
  sqrtPriceX64ToPrice,
  sqrtPriceX64ToSqrtPrice,
  adjustPriceForDecimals,
  calculateFullRangeAmounts,
  formatTokenAmount,
  calculateLiquidityShare,
  calculateTotalValueInB,
  calculateTotalValueInA,
} from "../utils/math";

// Configure BigNumber
BigNumber.config({
  DECIMAL_PLACES: 40,
  ROUNDING_MODE: BigNumber.ROUND_DOWN,
});

/**
 * OracleService provides comprehensive whirlpool and position data.
 */
export class OracleService {
  private connection: Connection;
  private whirlpoolService: WhirlpoolService;
  private positionService: PositionService;

  constructor(connection: Connection) {
    this.connection = connection;
    this.whirlpoolService = new WhirlpoolService(connection);
    this.positionService = new PositionService(connection);
  }

  /**
   * Get complete oracle data for a whirlpool.
   * 
   * Fetches whirlpool state, all positions, and computes valuations.
   * 
   * @param whirlpoolAddress - Whirlpool address
   * @param tokenADecimals - Optional decimals for token A (fetched if not provided)
   * @param tokenBDecimals - Optional decimals for token B (fetched if not provided)
   * @returns Complete OracleResponse with all computed data
   */
  async getOracleData(
    whirlpoolAddress: string,
    tokenADecimals?: number,
    tokenBDecimals?: number
  ): Promise<OracleResponse> {
    // Step 1: Fetch whirlpool data
    const whirlpool = await this.whirlpoolService.getWhirlpool(whirlpoolAddress);
    
    // Step 2: Fetch token decimals if not provided
    const [decimalsA, decimalsB] = await Promise.all([
      tokenADecimals ?? this.whirlpoolService.getTokenDecimals(whirlpool.tokenMintA),
      tokenBDecimals ?? this.whirlpoolService.getTokenDecimals(whirlpool.tokenMintB),
    ]);

    // Step 3: Compute price data
    const priceData = this.computePriceData(whirlpool, decimalsA, decimalsB);

    // Step 4: Fetch all positions
    const positions = await this.positionService.getPositionsForWhirlpool(
      whirlpoolAddress,
      whirlpool.tickSpacing
    );

    // Step 5: Compute valuations for each position
    const positionValuations = this.computePositionValuations(
      positions,
      whirlpool.sqrtPriceX64,
      priceData,
      decimalsA,
      decimalsB
    );

    // Step 6: Compute aggregates
    const aggregate = this.computeAggregates(
      positionValuations,
      positions,
      decimalsA,
      decimalsB
    );

    return {
      timestamp: Date.now(),
      whirlpool,
      price: priceData,
      tokenA: {
        mint: whirlpool.tokenMintA,
        decimals: decimalsA,
      },
      tokenB: {
        mint: whirlpool.tokenMintB,
        decimals: decimalsB,
      },
      positions: positionValuations,
      aggregate,
    };
  }

  /**
   * Get just the price data for a whirlpool.
   * 
   * @param whirlpoolAddress - Whirlpool address
   * @param tokenADecimals - Optional decimals for token A
   * @param tokenBDecimals - Optional decimals for token B
   * @returns PriceData with current price information
   */
  async getPrice(
    whirlpoolAddress: string,
    tokenADecimals?: number,
    tokenBDecimals?: number
  ): Promise<PriceData & { tokenMintA: string; tokenMintB: string }> {
    const whirlpool = await this.whirlpoolService.getWhirlpool(whirlpoolAddress);
    
    const [decimalsA, decimalsB] = await Promise.all([
      tokenADecimals ?? this.whirlpoolService.getTokenDecimals(whirlpool.tokenMintA),
      tokenBDecimals ?? this.whirlpoolService.getTokenDecimals(whirlpool.tokenMintB),
    ]);

    const priceData = this.computePriceData(whirlpool, decimalsA, decimalsB);

    return {
      ...priceData,
      tokenMintA: whirlpool.tokenMintA,
      tokenMintB: whirlpool.tokenMintB,
    };
  }

  /**
   * Get positions only for a whirlpool.
   * 
   * @param whirlpoolAddress - Whirlpool address
   * @param fullRangeOnly - If true, only return full-range positions
   * @returns Array of positions with valuations
   */
  async getPositions(
    whirlpoolAddress: string,
    fullRangeOnly: boolean = false
  ): Promise<PositionValuation[]> {
    const whirlpool = await this.whirlpoolService.getWhirlpool(whirlpoolAddress);
    
    const [decimalsA, decimalsB] = await Promise.all([
      this.whirlpoolService.getTokenDecimals(whirlpool.tokenMintA),
      this.whirlpoolService.getTokenDecimals(whirlpool.tokenMintB),
    ]);

    const priceData = this.computePriceData(whirlpool, decimalsA, decimalsB);

    let positions = await this.positionService.getPositionsForWhirlpool(
      whirlpoolAddress,
      whirlpool.tickSpacing
    );

    if (fullRangeOnly) {
      positions = this.positionService.filterFullRangePositions(positions);
    }

    return this.computePositionValuations(
      positions,
      whirlpool.sqrtPriceX64,
      priceData,
      decimalsA,
      decimalsB
    );
  }

  /**
   * Compute price data from whirlpool state.
   * 
   * @param whirlpool - Whirlpool data
   * @param decimalsA - Token A decimals
   * @param decimalsB - Token B decimals
   * @returns PriceData with all price representations
   */
  private computePriceData(
    whirlpool: WhirlpoolData,
    decimalsA: number,
    decimalsB: number
  ): PriceData {
    const sqrtPriceX64 = whirlpool.sqrtPriceX64;
    
    // sqrtPrice = sqrtPriceX64 / 2^64
    const sqrtPrice = sqrtPriceX64ToSqrtPrice(sqrtPriceX64);
    
    // priceRaw = sqrtPrice^2 (tokenB per tokenA in smallest units)
    const priceRaw = sqrtPriceX64ToPrice(sqrtPriceX64);
    
    // price = priceRaw * (10^decimalsA / 10^decimalsB)
    const price = adjustPriceForDecimals(priceRaw, decimalsA, decimalsB);
    
    // Inverse: tokenA per tokenB
    const inversePriceRaw = new BigNumber(1).dividedBy(priceRaw);
    const inversePrice = new BigNumber(1).dividedBy(price);

    return {
      priceRaw: priceRaw.toFixed(18),
      price: price.toFixed(18),
      inversePriceRaw: inversePriceRaw.toFixed(18),
      inversePrice: inversePrice.toFixed(18),
      sqrtPriceX64,
      sqrtPrice: sqrtPrice.toFixed(18),
      tickCurrentIndex: whirlpool.tickCurrentIndex,
    };
  }

  /**
   * Compute valuations for all positions.
   * 
   * @param positions - Array of position data
   * @param sqrtPriceX64 - Current sqrt price
   * @param priceData - Computed price data
   * @param decimalsA - Token A decimals
   * @param decimalsB - Token B decimals
   * @returns Array of position valuations
   */
  private computePositionValuations(
    positions: PositionData[],
    sqrtPriceX64: string,
    priceData: PriceData,
    decimalsA: number,
    decimalsB: number
  ): PositionValuation[] {
    const totalLiquidity = positions.reduce(
      (sum, p) => sum + BigInt(p.liquidity),
      0n
    );

    return positions.map(position => {
      // Compute token amounts using full-range formula
      const { amountA, amountB } = calculateFullRangeAmounts(
        position.liquidity,
        sqrtPriceX64
      );

      // Format human-readable amounts
      const amountAFormatted = formatTokenAmount(amountA, decimalsA);
      const amountBFormatted = formatTokenAmount(amountB, decimalsB);

      // Calculate liquidity share
      const liquidityShare = calculateLiquidityShare(
        position.liquidity,
        totalLiquidity.toString()
      );

      // Calculate total value in tokenB terms
      const priceRaw = new BigNumber(priceData.priceRaw);
      const totalValueInB = calculateTotalValueInB(amountA, amountB, priceRaw);
      const totalValueInBFormatted = formatTokenAmount(totalValueInB, decimalsB);

      // Format fee amounts
      const feeOwedAFormatted = formatTokenAmount(position.feeOwedA, decimalsA);
      const feeOwedBFormatted = formatTokenAmount(position.feeOwedB, decimalsB);

      return {
        positionAddress: position.address,
        positionMint: position.positionMint,
        amountARaw: amountA.integerValue().toFixed(),
        amountBRaw: amountB.integerValue().toFixed(),
        amountA: amountAFormatted,
        amountB: amountBFormatted,
        liquidity: position.liquidity,
        liquidityShare,
        totalValueInB: totalValueInBFormatted,
        tickLowerIndex: position.tickLowerIndex,
        tickUpperIndex: position.tickUpperIndex,
        isFullRange: position.isFullRange,
        feeOwedA: feeOwedAFormatted,
        feeOwedB: feeOwedBFormatted,
      };
    });
  }

  /**
   * Compute aggregate metrics across all positions.
   * 
   * @param valuations - Array of position valuations
   * @param positions - Original position data
   * @param decimalsA - Token A decimals
   * @param decimalsB - Token B decimals
   * @returns Aggregate metrics
   */
  private computeAggregates(
    valuations: PositionValuation[],
    positions: PositionData[],
    decimalsA: number,
    decimalsB: number
  ): OracleResponse["aggregate"] {
    // Count positions
    const totalPositions = positions.length;
    const fullRangePositions = positions.filter(p => p.isFullRange).length;

    // Sum liquidity
    const totalLiquidity = positions.reduce(
      (sum, p) => sum + BigInt(p.liquidity),
      0n
    );

    // Sum token amounts (raw)
    const totalAmountARaw = valuations.reduce(
      (sum, v) => sum.plus(v.amountARaw),
      new BigNumber(0)
    );
    const totalAmountBRaw = valuations.reduce(
      (sum, v) => sum.plus(v.amountBRaw),
      new BigNumber(0)
    );

    // Format total amounts
    const totalAmountA = formatTokenAmount(totalAmountARaw, decimalsA);
    const totalAmountB = formatTokenAmount(totalAmountBRaw, decimalsB);

    // Calculate TVL in both tokens
    const tvlInB = valuations.reduce(
      (sum, v) => sum.plus(v.totalValueInB),
      new BigNumber(0)
    );

    // For TVL in A, we need inverse calculation
    // We compute it from raw amounts to avoid precision loss
    const tvlInA = valuations.reduce((sum, v) => {
      const amountA = new BigNumber(v.amountARaw);
      const amountB = new BigNumber(v.amountBRaw);
      // Assuming price ratio is consistent, use inverse
      // TVL in A = amountA + amountB / price
      // But since we're summing, just use 2 * amountA as approximation for full-range
      // Actually, for full range: totalValueInA = 2 * amountA (symmetric)
      return sum.plus(amountA.multipliedBy(2));
    }, new BigNumber(0));

    // Liquidity distribution
    const liquidityDistribution = valuations.map(v => ({
      positionAddress: v.positionAddress,
      share: v.liquidityShare,
    }));

    return {
      totalPositions,
      fullRangePositions,
      totalLiquidity: totalLiquidity.toString(),
      totalAmountA,
      totalAmountB,
      totalAmountARaw: totalAmountARaw.toFixed(),
      totalAmountBRaw: totalAmountBRaw.toFixed(),
      tvlInB: tvlInB.toFixed(decimalsB),
      tvlInA: formatTokenAmount(tvlInA, decimalsA),
      liquidityDistribution,
    };
  }
}
