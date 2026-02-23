/**
 * ============================================================================
 * POSITION SERVICE
 * ============================================================================
 * 
 * Fetches and decodes all Position accounts for a given Whirlpool.
 * 
 * POSITION ACCOUNT STRUCTURE:
 * ───────────────────────────
 * Whirlpool Position accounts are PDAs derived from the position mint NFT:
 * - Seeds: ["position", position_mint]
 * - Program: whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc
 * 
 * Position data layout (after 8-byte discriminator):
 * - whirlpool: Pubkey (32 bytes)
 * - position_mint: Pubkey (32 bytes)
 * - liquidity: u128 (16 bytes)
 * - tick_lower_index: i32 (4 bytes)
 * - tick_upper_index: i32 (4 bytes)
 * - fee_growth_checkpoint_a: u128 (16 bytes)
 * - fee_owed_a: u64 (8 bytes)
 * - fee_growth_checkpoint_b: u128 (16 bytes)
 * - fee_owed_b: u64 (8 bytes)
 * - reward_infos: [PositionRewardInfo; 3] (3 * 24 = 72 bytes)
 * 
 * Total: 8 + 32 + 32 + 16 + 4 + 4 + 16 + 8 + 16 + 8 + 72 = 216 bytes
 * 
 * FETCHING STRATEGY:
 * ──────────────────
 * We use getProgramAccounts with a memcmp filter on the whirlpool field:
 * - Filter offset: 8 (after discriminator)
 * - Filter bytes: whirlpool pubkey (32 bytes)
 * 
 * This efficiently retrieves only positions for our target whirlpool.
 */

import { Connection, PublicKey, GetProgramAccountsFilter } from "@solana/web3.js";
import { PositionData, WHIRLPOOL_PROGRAM_ID, MIN_TICK_INDEX, MAX_TICK_INDEX } from "../types";
import { readU128, readU64, readI32, isFullRangePosition } from "../utils/math";

/**
 * Position account layout offsets (after 8-byte discriminator)
 */
const DISCRIMINATOR_SIZE = 8;

const POSITION_OFFSETS = {
  whirlpool: DISCRIMINATOR_SIZE + 0,              // Pubkey, 32 bytes
  positionMint: DISCRIMINATOR_SIZE + 32,          // Pubkey, 32 bytes
  liquidity: DISCRIMINATOR_SIZE + 64,             // u128, 16 bytes
  tickLowerIndex: DISCRIMINATOR_SIZE + 80,        // i32, 4 bytes
  tickUpperIndex: DISCRIMINATOR_SIZE + 84,        // i32, 4 bytes
  feeGrowthCheckpointA: DISCRIMINATOR_SIZE + 88,  // u128, 16 bytes
  feeOwedA: DISCRIMINATOR_SIZE + 104,             // u64, 8 bytes
  feeGrowthCheckpointB: DISCRIMINATOR_SIZE + 112, // u128, 16 bytes
  feeOwedB: DISCRIMINATOR_SIZE + 128,             // u64, 8 bytes
  // reward_infos: DISCRIMINATOR_SIZE + 136       // [PositionRewardInfo; 3], 72 bytes
};

/**
 * Position account discriminator for Whirlpool positions.
 * This is the first 8 bytes of sha256("account:Position")
 */
const POSITION_DISCRIMINATOR = Buffer.from([
  170, 188, 143, 228, 122, 64, 247, 208
]);

/**
 * PositionService handles fetching and decoding position data.
 */
export class PositionService {
  private connection: Connection;
  private whirlpoolProgramId: PublicKey;

  constructor(connection: Connection) {
    this.connection = connection;
    this.whirlpoolProgramId = new PublicKey(WHIRLPOOL_PROGRAM_ID);
  }

  /**
   * Fetch all Position accounts for a given Whirlpool.
   * 
   * Uses getProgramAccounts with memcmp filter to efficiently
   * retrieve only positions belonging to the target whirlpool.
   * 
   * @param whirlpoolAddress - The Whirlpool address to fetch positions for
   * @param tickSpacing - Whirlpool tick spacing (for full-range detection)
   * @returns Array of decoded PositionData
   */
  async getPositionsForWhirlpool(
    whirlpoolAddress: string | PublicKey,
    tickSpacing: number
  ): Promise<PositionData[]> {
    const whirlpoolPubkey = typeof whirlpoolAddress === "string"
      ? new PublicKey(whirlpoolAddress)
      : whirlpoolAddress;

    // Build filters:
    // 1. Filter by account size (Position accounts are 216 bytes)
    // 2. Filter by whirlpool address at offset 8 (after discriminator)
    const filters: GetProgramAccountsFilter[] = [
      {
        dataSize: 216, // Position account size
      },
      {
        memcmp: {
          offset: POSITION_OFFSETS.whirlpool,
          bytes: whirlpoolPubkey.toBase58(),
        },
      },
    ];

    // Fetch all matching accounts
    const accounts = await this.connection.getProgramAccounts(
      this.whirlpoolProgramId,
      { filters }
    );

    // Decode each position
    const positions: PositionData[] = [];
    
    for (const { pubkey, account } of accounts) {
      try {
        const position = this.decodePosition(pubkey, account.data, tickSpacing);
        positions.push(position);
      } catch (error) {
        console.error(`Error decoding position ${pubkey.toBase58()}:`, error);
      }
    }

    return positions;
  }

  /**
   * Fetch a single Position account by its address.
   * 
   * @param positionAddress - The Position PDA address
   * @param tickSpacing - Whirlpool tick spacing (for full-range detection)
   * @returns Decoded PositionData
   */
  async getPosition(
    positionAddress: string | PublicKey,
    tickSpacing: number
  ): Promise<PositionData> {
    const address = typeof positionAddress === "string"
      ? new PublicKey(positionAddress)
      : positionAddress;

    const accountInfo = await this.connection.getAccountInfo(address);

    if (!accountInfo) {
      throw new Error(`Position account not found: ${address.toBase58()}`);
    }

    if (!accountInfo.owner.equals(this.whirlpoolProgramId)) {
      throw new Error(
        `Invalid account owner. Expected ${WHIRLPOOL_PROGRAM_ID}, got ${accountInfo.owner.toBase58()}`
      );
    }

    return this.decodePosition(address, accountInfo.data, tickSpacing);
  }

  /**
   * Derive Position PDA from position mint.
   * 
   * Seeds: ["position", position_mint]
   * 
   * @param positionMint - The position NFT mint address
   * @returns Position PDA address
   */
  derivePositionAddress(positionMint: PublicKey): PublicKey {
    const [positionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("position"), positionMint.toBuffer()],
      this.whirlpoolProgramId
    );
    return positionPda;
  }

  /**
   * Decode raw Position account data.
   * 
   * @param address - Position PDA address
   * @param data - Raw account data buffer
   * @param tickSpacing - Whirlpool tick spacing
   * @returns Decoded PositionData
   */
  private decodePosition(
    address: PublicKey,
    data: Buffer,
    tickSpacing: number
  ): PositionData {
    // Verify discriminator
    const discriminator = data.subarray(0, DISCRIMINATOR_SIZE);
    if (!discriminator.equals(POSITION_DISCRIMINATOR)) {
      throw new Error(`Invalid position discriminator for ${address.toBase58()}`);
    }

    // Parse fields
    const whirlpool = new PublicKey(
      data.subarray(POSITION_OFFSETS.whirlpool, POSITION_OFFSETS.whirlpool + 32)
    );
    
    const positionMint = new PublicKey(
      data.subarray(POSITION_OFFSETS.positionMint, POSITION_OFFSETS.positionMint + 32)
    );
    
    const liquidity = readU128(data, POSITION_OFFSETS.liquidity);
    const tickLowerIndex = readI32(data, POSITION_OFFSETS.tickLowerIndex);
    const tickUpperIndex = readI32(data, POSITION_OFFSETS.tickUpperIndex);
    
    const feeGrowthCheckpointA = readU128(data, POSITION_OFFSETS.feeGrowthCheckpointA);
    const feeOwedA = readU64(data, POSITION_OFFSETS.feeOwedA);
    
    const feeGrowthCheckpointB = readU128(data, POSITION_OFFSETS.feeGrowthCheckpointB);
    const feeOwedB = readU64(data, POSITION_OFFSETS.feeOwedB);

    // Check if this is a full-range position
    const isFullRange = isFullRangePosition(tickLowerIndex, tickUpperIndex, tickSpacing);

    return {
      address: address.toBase58(),
      whirlpool: whirlpool.toBase58(),
      positionMint: positionMint.toBase58(),
      liquidity: liquidity.toString(),
      tickLowerIndex,
      tickUpperIndex,
      feeGrowthCheckpointA: feeGrowthCheckpointA.toString(),
      feeGrowthCheckpointB: feeGrowthCheckpointB.toString(),
      feeOwedA: feeOwedA.toString(),
      feeOwedB: feeOwedB.toString(),
      isFullRange,
    };
  }

  /**
   * Filter positions to only full-range positions.
   * 
   * @param positions - Array of positions to filter
   * @returns Array of full-range positions only
   */
  filterFullRangePositions(positions: PositionData[]): PositionData[] {
    return positions.filter(p => p.isFullRange);
  }

  /**
   * Get total liquidity across all positions.
   * 
   * @param positions - Array of positions
   * @returns Total liquidity as bigint
   */
  getTotalLiquidity(positions: PositionData[]): bigint {
    return positions.reduce(
      (sum, p) => sum + BigInt(p.liquidity),
      0n
    );
  }
}
