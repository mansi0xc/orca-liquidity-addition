/**
 * ============================================================================
 * WHIRLPOOL SERVICE
 * ============================================================================
 * 
 * Fetches and decodes Orca Whirlpool account data from on-chain.
 * 
 * ACCOUNT STRUCTURE:
 * ──────────────────
 * Whirlpool accounts use Anchor serialization:
 * - 8-byte discriminator (Anchor)
 * - Followed by struct fields in order
 * 
 * Key fields we need:
 * - sqrtPriceX64: Current price as sqrt in Q64.64 format
 * - tickCurrentIndex: Current tick
 * - liquidity: Active liquidity
 * - tokenMintA/B: Token mints
 * - tokenVaultA/B: Token vault addresses
 */

import { Connection, PublicKey, AccountInfo } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";
import { WhirlpoolData, WHIRLPOOL_PROGRAM_ID } from "../types";
import { readU128, readU64, readU16, readI32 } from "../utils/math";

/**
 * Whirlpool account layout offsets (after 8-byte discriminator)
 * 
 * From Orca Whirlpool source:
 * pub struct Whirlpool {
 *     pub whirlpools_config: Pubkey,      // 32 bytes, offset 0
 *     pub whirlpool_bump: [u8; 1],        // 1 byte, offset 32
 *     pub tick_spacing: u16,               // 2 bytes, offset 33
 *     pub tick_spacing_seed: [u8; 2],     // 2 bytes, offset 35
 *     pub fee_rate: u16,                   // 2 bytes, offset 37
 *     pub protocol_fee_rate: u16,          // 2 bytes, offset 39
 *     pub liquidity: u128,                 // 16 bytes, offset 41
 *     pub sqrt_price: u128,                // 16 bytes, offset 57
 *     pub tick_current_index: i32,         // 4 bytes, offset 73
 *     pub protocol_fee_owed_a: u64,        // 8 bytes, offset 77
 *     pub protocol_fee_owed_b: u64,        // 8 bytes, offset 85
 *     pub token_mint_a: Pubkey,            // 32 bytes, offset 93
 *     pub token_vault_a: Pubkey,           // 32 bytes, offset 125
 *     pub fee_growth_global_a: u128,       // 16 bytes, offset 157
 *     pub token_mint_b: Pubkey,            // 32 bytes, offset 173
 *     pub token_vault_b: Pubkey,           // 32 bytes, offset 205
 *     pub fee_growth_global_b: u128,       // 16 bytes, offset 237
 *     pub reward_last_updated_timestamp: u64, // 8 bytes, offset 253
 *     pub reward_infos: [WhirlpoolRewardInfo; 3], // 3 * 128 bytes = 384 bytes
 * }
 */
const DISCRIMINATOR_SIZE = 8;

const WHIRLPOOL_OFFSETS = {
  whirlpoolsConfig: DISCRIMINATOR_SIZE + 0,      // Pubkey, 32 bytes
  whirlpoolBump: DISCRIMINATOR_SIZE + 32,        // [u8; 1], 1 byte
  tickSpacing: DISCRIMINATOR_SIZE + 33,          // u16, 2 bytes
  tickSpacingSeed: DISCRIMINATOR_SIZE + 35,      // [u8; 2], 2 bytes
  feeRate: DISCRIMINATOR_SIZE + 37,              // u16, 2 bytes
  protocolFeeRate: DISCRIMINATOR_SIZE + 39,      // u16, 2 bytes
  liquidity: DISCRIMINATOR_SIZE + 41,            // u128, 16 bytes
  sqrtPrice: DISCRIMINATOR_SIZE + 57,            // u128, 16 bytes
  tickCurrentIndex: DISCRIMINATOR_SIZE + 73,     // i32, 4 bytes
  protocolFeeOwedA: DISCRIMINATOR_SIZE + 77,     // u64, 8 bytes
  protocolFeeOwedB: DISCRIMINATOR_SIZE + 85,     // u64, 8 bytes
  tokenMintA: DISCRIMINATOR_SIZE + 93,           // Pubkey, 32 bytes
  tokenVaultA: DISCRIMINATOR_SIZE + 125,         // Pubkey, 32 bytes
  feeGrowthGlobalA: DISCRIMINATOR_SIZE + 157,    // u128, 16 bytes
  tokenMintB: DISCRIMINATOR_SIZE + 173,          // Pubkey, 32 bytes
  tokenVaultB: DISCRIMINATOR_SIZE + 205,         // Pubkey, 32 bytes
  feeGrowthGlobalB: DISCRIMINATOR_SIZE + 237,    // u128, 16 bytes
};

/**
 * WhirlpoolService handles fetching and decoding whirlpool data.
 */
export class WhirlpoolService {
  private connection: Connection;
  private whirlpoolProgramId: PublicKey;

  constructor(connection: Connection) {
    this.connection = connection;
    this.whirlpoolProgramId = new PublicKey(WHIRLPOOL_PROGRAM_ID);
  }

  /**
   * Fetch and decode a Whirlpool account.
   * 
   * @param whirlpoolAddress - The Whirlpool address to fetch
   * @returns Decoded WhirlpoolData
   * @throws Error if account doesn't exist or can't be decoded
   */
  async getWhirlpool(whirlpoolAddress: string | PublicKey): Promise<WhirlpoolData> {
    const address = typeof whirlpoolAddress === "string" 
      ? new PublicKey(whirlpoolAddress) 
      : whirlpoolAddress;

    // Fetch account data
    const accountInfo = await this.connection.getAccountInfo(address);
    
    if (!accountInfo) {
      throw new Error(`Whirlpool account not found: ${address.toBase58()}`);
    }

    // Verify owner
    if (!accountInfo.owner.equals(this.whirlpoolProgramId)) {
      throw new Error(
        `Invalid account owner. Expected ${WHIRLPOOL_PROGRAM_ID}, got ${accountInfo.owner.toBase58()}`
      );
    }

    // Decode the account data
    const whirlpoolData = this.decodeWhirlpool(address, accountInfo);

    // Fetch vault balances
    const [vaultBalanceA, vaultBalanceB] = await Promise.all([
      this.getTokenBalance(whirlpoolData.tokenVaultA),
      this.getTokenBalance(whirlpoolData.tokenVaultB),
    ]);

    return {
      ...whirlpoolData,
      vaultBalanceA,
      vaultBalanceB,
    };
  }

  /**
   * Decode raw Whirlpool account data.
   * 
   * @param address - Whirlpool address
   * @param accountInfo - Raw account info
   * @returns Partially decoded WhirlpoolData (without vault balances)
   */
  private decodeWhirlpool(
    address: PublicKey,
    accountInfo: AccountInfo<Buffer>
  ): Omit<WhirlpoolData, "vaultBalanceA" | "vaultBalanceB"> {
    const data = accountInfo.data;

    // Parse all fields from buffer
    const whirlpoolsConfig = new PublicKey(
      data.subarray(WHIRLPOOL_OFFSETS.whirlpoolsConfig, WHIRLPOOL_OFFSETS.whirlpoolsConfig + 32)
    );
    
    const whirlpoolBump = data[WHIRLPOOL_OFFSETS.whirlpoolBump];
    const tickSpacing = readU16(data, WHIRLPOOL_OFFSETS.tickSpacing);
    const feeRate = readU16(data, WHIRLPOOL_OFFSETS.feeRate);
    const protocolFeeRate = readU16(data, WHIRLPOOL_OFFSETS.protocolFeeRate);
    
    const liquidity = readU128(data, WHIRLPOOL_OFFSETS.liquidity);
    const sqrtPriceX64 = readU128(data, WHIRLPOOL_OFFSETS.sqrtPrice);
    const tickCurrentIndex = readI32(data, WHIRLPOOL_OFFSETS.tickCurrentIndex);
    
    const protocolFeeOwedA = readU64(data, WHIRLPOOL_OFFSETS.protocolFeeOwedA);
    const protocolFeeOwedB = readU64(data, WHIRLPOOL_OFFSETS.protocolFeeOwedB);
    
    const tokenMintA = new PublicKey(
      data.subarray(WHIRLPOOL_OFFSETS.tokenMintA, WHIRLPOOL_OFFSETS.tokenMintA + 32)
    );
    const tokenVaultA = new PublicKey(
      data.subarray(WHIRLPOOL_OFFSETS.tokenVaultA, WHIRLPOOL_OFFSETS.tokenVaultA + 32)
    );
    const feeGrowthGlobalA = readU128(data, WHIRLPOOL_OFFSETS.feeGrowthGlobalA);
    
    const tokenMintB = new PublicKey(
      data.subarray(WHIRLPOOL_OFFSETS.tokenMintB, WHIRLPOOL_OFFSETS.tokenMintB + 32)
    );
    const tokenVaultB = new PublicKey(
      data.subarray(WHIRLPOOL_OFFSETS.tokenVaultB, WHIRLPOOL_OFFSETS.tokenVaultB + 32)
    );
    const feeGrowthGlobalB = readU128(data, WHIRLPOOL_OFFSETS.feeGrowthGlobalB);

    return {
      address: address.toBase58(),
      whirlpoolsConfig: whirlpoolsConfig.toBase58(),
      whirlpoolBump,
      tickSpacing,
      feeRate,
      protocolFeeRate,
      sqrtPriceX64: sqrtPriceX64.toString(),
      tickCurrentIndex,
      liquidity: liquidity.toString(),
      tokenMintA: tokenMintA.toBase58(),
      tokenMintB: tokenMintB.toBase58(),
      tokenVaultA: tokenVaultA.toBase58(),
      tokenVaultB: tokenVaultB.toBase58(),
      feeGrowthGlobalA: feeGrowthGlobalA.toString(),
      feeGrowthGlobalB: feeGrowthGlobalB.toString(),
      protocolFeeOwedA: protocolFeeOwedA.toString(),
      protocolFeeOwedB: protocolFeeOwedB.toString(),
    };
  }

  /**
   * Get token balance for a vault account.
   * 
   * @param vaultAddress - Token account address
   * @returns Balance as string
   */
  private async getTokenBalance(vaultAddress: string): Promise<string> {
    try {
      const account = await getAccount(
        this.connection,
        new PublicKey(vaultAddress)
      );
      return account.amount.toString();
    } catch (error) {
      console.error(`Error fetching vault balance for ${vaultAddress}:`, error);
      return "0";
    }
  }

  /**
   * Get token decimals for a mint.
   * 
   * @param mintAddress - Token mint address
   * @returns Decimals as number
   */
  async getTokenDecimals(mintAddress: string | PublicKey): Promise<number> {
    const address = typeof mintAddress === "string"
      ? new PublicKey(mintAddress)
      : mintAddress;

    const mintInfo = await this.connection.getParsedAccountInfo(address);
    
    if (!mintInfo.value) {
      throw new Error(`Mint not found: ${address.toBase58()}`);
    }

    const data = mintInfo.value.data;
    if ("parsed" in data) {
      return data.parsed.info.decimals;
    }

    throw new Error(`Could not parse mint data for: ${address.toBase58()}`);
  }

  /**
   * Get tick spacing for a whirlpool.
   * Useful for calculating full-range tick bounds.
   * 
   * @param whirlpoolAddress - Whirlpool address
   * @returns Tick spacing
   */
  async getTickSpacing(whirlpoolAddress: string | PublicKey): Promise<number> {
    const whirlpool = await this.getWhirlpool(whirlpoolAddress);
    return whirlpool.tickSpacing;
  }
}
