import express from "express";
import helmet from "helmet";
import cors from "cors";
import * as dotenv from "dotenv";
import { Connection, PublicKey, Keypair } from "@solana/web3.js";
import * as nacl from "tweetnacl";

dotenv.config();

// =============================================================================
// LOGGING
// =============================================================================

function log(level: string, message: string, meta?: Record<string, unknown>) {
  const entry = {
    timestamp: new Date().toISOString(),
    level,
    message,
    ...meta,
  };
  console.log(JSON.stringify(entry));
}

// =============================================================================
// APP SETUP
// =============================================================================

const app = express();
app.use(helmet());

const allowedOrigins = process.env.ALLOWED_ORIGINS
  ? process.env.ALLOWED_ORIGINS.split(",")
  : ["http://localhost:3000"];

app.use(
  cors({
    origin: function (origin, callback) {
      if (!origin) return callback(null, true);
      if (allowedOrigins.indexOf(origin) === -1) {
        return callback(new Error("CORS policy: Not allowed"), false);
      }
      return callback(null, true);
    },
    credentials: true,
  })
);

app.use(express.json({ limit: "10mb" }));

// =============================================================================
// SOLANA CONNECTION
// =============================================================================

const RPC_URL = process.env.RPC_URL || "https://api.devnet.solana.com";
const connection = new Connection(RPC_URL, { commitment: "confirmed" });

// =============================================================================
// ORACLE KEYPAIR
// =============================================================================

function getOracleKeypair(): Keypair {
  const key = process.env.ORACLE_PRIVATE_KEY;
  if (!key) throw new Error("ORACLE_PRIVATE_KEY env var is required");
  const secretKey = Buffer.from(key, "base64");
  if (secretKey.length !== 64) throw new Error("ORACLE_PRIVATE_KEY must be 64 bytes base64");
  return Keypair.fromSecretKey(secretKey);
}

// =============================================================================
// CHAIN_CONFIG — maps tokenId (position mint) to pool data
//
// Solana equivalent of the EVM CHAIN_CONFIG.
// tokenId = Orca position mint pubkey (string)
// poolAddress = Orca Whirlpool address
// positionManager = Whirlpool program ID (always the same on Solana)
// =============================================================================

interface PoolConfig {
  name: string;
  poolAddress: string;
  positionManager: string;
}

const CHAIN_CONFIG: Record<string, PoolConfig> = {
  // SOL-GMI
  GdcbPDTGxidg2Q5ACYhv7Q5q3AoG1t5FKD83HUEqC6dv: {
    name: "SOL-GMI",
    poolAddress: "8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN",
    positionManager: "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
  },
  // SOL-GMI-L1
  "7YiaSGsA3uTekuWkqpoDS7Z7RhyKF8bSV4DbjXzEXsDz": {
    name: "SOL-GMI-L1",
    poolAddress: "36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V",
    positionManager: "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
  },
  // SOL-GMI-L2
  Aw7nyA5zazPuDPdTU1c1HJuimQK5yhkpu25PQbDkik7W: {
    name: "SOL-GMI-L2",
    poolAddress: "GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ",
    positionManager: "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
  },
  // SOL-GMI-L3
  "9YhETyPjFdJcHMK7bSQsWC1xpVAdV5um8yWkTmE2mMf2": {
    name: "SOL-GMI-L3",
    poolAddress: "2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo",
    positionManager: "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc",
  },
};

// =============================================================================
// ON-CHAIN ACCOUNT SIZES
// =============================================================================

const WHIRLPOOL_ACCOUNT_SIZE = 653;
const POSITION_ACCOUNT_SIZE = 216;

// =============================================================================
// SIGNATURE CONSTANTS (must match lp-bonds program constants.rs + ed25519.rs)
// =============================================================================

const SIGNATURE_DOMAIN = "LP_BONDS_SOLANA_V1"; // 18 bytes
const CANONICAL_MESSAGE_LEN = 198;
const LP_BONDS_PROGRAM_ID = process.env.LP_BONDS_PROGRAM_ID || "AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w";

// =============================================================================
// BINARY HELPERS — read little-endian values from Buffer
// =============================================================================

function readU16(buf: Buffer, offset: number): number {
  return buf.readUInt16LE(offset);
}

function readI32(buf: Buffer, offset: number): number {
  return buf.readInt32LE(offset);
}

function readU64(buf: Buffer, offset: number): bigint {
  return buf.readBigUInt64LE(offset);
}

function readU128(buf: Buffer, offset: number): bigint {
  const lo = buf.readBigUInt64LE(offset);
  const hi = buf.readBigUInt64LE(offset + 8);
  return (hi << 64n) | lo;
}

// =============================================================================
// WHIRLPOOL DECODING (653 bytes)
// =============================================================================

interface WhirlpoolState {
  sqrtPriceX64: bigint;
  tickCurrentIndex: number;
  tickSpacing: number;
  liquidity: bigint;
  tokenMintA: PublicKey;
  tokenMintB: PublicKey;
  feeRate: number;
}

function decodeWhirlpool(data: Buffer): WhirlpoolState {
  let o = 8; // skip discriminator
  // +0  whirlpools_config (32)
  o += 32;
  // +32 whirlpool_bump (1)
  o += 1;
  // +33 tick_spacing (u16)
  const tickSpacing = readU16(data, o);
  o += 2;
  // +35 tick_spacing_seed (2)
  o += 2;
  // +37 fee_rate (u16)
  const feeRate = readU16(data, o);
  o += 2;
  // +39 protocol_fee_rate (u16)
  o += 2;
  // +41 liquidity (u128)
  const liquidity = readU128(data, o);
  o += 16;
  // +57 sqrt_price (u128)
  const sqrtPriceX64 = readU128(data, o);
  o += 16;
  // +73 tick_current_index (i32)
  const tickCurrentIndex = readI32(data, o);
  o += 4;
  // +77 protocol_fee_owed_a (u64)
  o += 8;
  // +85 protocol_fee_owed_b (u64)
  o += 8;
  // +93 token_mint_a (32)
  const tokenMintA = new PublicKey(data.subarray(o, o + 32));
  o += 32;
  // +125 token_vault_a (32)
  o += 32;
  // +157 fee_growth_global_a (u128)
  o += 16;
  // +173 token_mint_b (32)
  const tokenMintB = new PublicKey(data.subarray(o, o + 32));

  return { sqrtPriceX64, tickCurrentIndex, tickSpacing, liquidity, tokenMintA, tokenMintB, feeRate };
}

// =============================================================================
// ORCA POSITION DECODING (216 bytes)
//
// PDA: ["position", position_mint] on Whirlpool program
//
// Layout (after 8-byte discriminator):
//   +0  whirlpool           Pubkey (32)
//  +32  position_mint       Pubkey (32)
//  +64  liquidity           u128   (16)
//  +80  tick_lower_index    i32    ( 4)
//  +84  tick_upper_index    i32    ( 4)
//  +88  fee_growth_ckpt_a   u128   (16)
// +104  fee_owed_a          u64    ( 8)
// +112  fee_growth_ckpt_b   u128   (16)
// +128  fee_owed_b          u64    ( 8)
// +136  reward_infos        [3*24] (72)
// =============================================================================

interface PositionState {
  whirlpool: PublicKey;
  positionMint: PublicKey;
  liquidity: bigint;
  tickLowerIndex: number;
  tickUpperIndex: number;
}

function decodePosition(data: Buffer): PositionState {
  let o = 8; // skip discriminator
  const whirlpool = new PublicKey(data.subarray(o, o + 32)); o += 32;
  const positionMint = new PublicKey(data.subarray(o, o + 32)); o += 32;
  const liquidity = readU128(data, o); o += 16;
  const tickLowerIndex = readI32(data, o); o += 4;
  const tickUpperIndex = readI32(data, o);

  return { whirlpool, positionMint, liquidity, tickLowerIndex, tickUpperIndex };
}

// =============================================================================
// POSITION AMOUNT MATH — Orca Whirlpool concentrated liquidity
//
// sqrtPriceX64 = sqrt(price) * 2^64  (Q64.64 fixed point)
// This is the Solana Orca equivalent of Uniswap V3's sqrtPriceX96 (Q96.96)
// =============================================================================

const Q64_BI = 1n << 64n;
const MIN_TICK = -443636;
const MAX_TICK = 443636;

// =============================================================================
// TICK → sqrtPriceX64 — Orca lookup-table approach (O(19) multiplications)
//
// Ported from Orca Whirlpool SDK PriceMath.tickIndexToSqrtPriceX64
// Uses precomputed constants per bit of the tick value.
// =============================================================================

function tickToSqrtPriceX64(tick: number): bigint {
  if (tick >= 0) {
    return tickToSqrtPricePositive(tick);
  } else {
    return tickToSqrtPriceNegative(tick);
  }
}

function tickToSqrtPricePositive(tick: number): bigint {
  // Constants are in Q96 format, final result shifted right by 32 to get Q64
  let ratio: bigint;
  if ((tick & 1) !== 0) {
    ratio = 79232123823359799118286999567n;
  } else {
    ratio = 79228162514264337593543950336n; // 1 << 96
  }
  if ((tick & 2) !== 0) ratio = (ratio * 79236085330515764027303304731n) >> 96n;
  if ((tick & 4) !== 0) ratio = (ratio * 79244008939048815603706035061n) >> 96n;
  if ((tick & 8) !== 0) ratio = (ratio * 79259858533276714757314932305n) >> 96n;
  if ((tick & 16) !== 0) ratio = (ratio * 79291567232598584799939703904n) >> 96n;
  if ((tick & 32) !== 0) ratio = (ratio * 79355022692464371645785046466n) >> 96n;
  if ((tick & 64) !== 0) ratio = (ratio * 79482085999252804386437311141n) >> 96n;
  if ((tick & 128) !== 0) ratio = (ratio * 79736823300114093921829183326n) >> 96n;
  if ((tick & 256) !== 0) ratio = (ratio * 80248749790819932309965073892n) >> 96n;
  if ((tick & 512) !== 0) ratio = (ratio * 81282483887344747381513967011n) >> 96n;
  if ((tick & 1024) !== 0) ratio = (ratio * 83390072131320151908154831281n) >> 96n;
  if ((tick & 2048) !== 0) ratio = (ratio * 87770609709833776024991924138n) >> 96n;
  if ((tick & 4096) !== 0) ratio = (ratio * 97234110755111693312479820773n) >> 96n;
  if ((tick & 8192) !== 0) ratio = (ratio * 119332217159966728226237229890n) >> 96n;
  if ((tick & 16384) !== 0) ratio = (ratio * 179736315981702064433883588727n) >> 96n;
  if ((tick & 32768) !== 0) ratio = (ratio * 407748233172238350107850275304n) >> 96n;
  if ((tick & 65536) !== 0) ratio = (ratio * 2098478828474011932436660412517n) >> 96n;
  if ((tick & 131072) !== 0) ratio = (ratio * 55581415166113811149459800483533n) >> 96n;
  if ((tick & 262144) !== 0) ratio = (ratio * 38992368544603139932233054999993551n) >> 96n;
  return ratio >> 32n; // Q96 → Q64
}

function tickToSqrtPriceNegative(tickIndex: number): bigint {
  // Constants are already in Q64 format
  const tick = Math.abs(tickIndex);
  let ratio: bigint;
  if ((tick & 1) !== 0) {
    ratio = 18445821805675392311n;
  } else {
    ratio = 18446744073709551616n; // 1 << 64
  }
  if ((tick & 2) !== 0) ratio = (ratio * 18444899583751176498n) >> 64n;
  if ((tick & 4) !== 0) ratio = (ratio * 18443055278223354162n) >> 64n;
  if ((tick & 8) !== 0) ratio = (ratio * 18439367220385604838n) >> 64n;
  if ((tick & 16) !== 0) ratio = (ratio * 18431993317065449817n) >> 64n;
  if ((tick & 32) !== 0) ratio = (ratio * 18417254355718160513n) >> 64n;
  if ((tick & 64) !== 0) ratio = (ratio * 18387811781193591352n) >> 64n;
  if ((tick & 128) !== 0) ratio = (ratio * 18329067761203520168n) >> 64n;
  if ((tick & 256) !== 0) ratio = (ratio * 18212142134806087854n) >> 64n;
  if ((tick & 512) !== 0) ratio = (ratio * 17980523815641551639n) >> 64n;
  if ((tick & 1024) !== 0) ratio = (ratio * 17526086738831147013n) >> 64n;
  if ((tick & 2048) !== 0) ratio = (ratio * 16651378430235024244n) >> 64n;
  if ((tick & 4096) !== 0) ratio = (ratio * 15030750278693429944n) >> 64n;
  if ((tick & 8192) !== 0) ratio = (ratio * 12247334978882834399n) >> 64n;
  if ((tick & 16384) !== 0) ratio = (ratio * 8131365268884726200n) >> 64n;
  if ((tick & 32768) !== 0) ratio = (ratio * 3584323654723342297n) >> 64n;
  if ((tick & 65536) !== 0) ratio = (ratio * 696457651847595233n) >> 64n;
  if ((tick & 131072) !== 0) ratio = (ratio * 26294789957452057n) >> 64n;
  if ((tick & 262144) !== 0) ratio = (ratio * 37481735321082n) >> 64n;
  return ratio;
}

function isFullRange(tickLower: number, tickUpper: number, tickSpacing: number): boolean {
  const minAligned = Math.ceil(MIN_TICK / tickSpacing) * tickSpacing;
  const maxAligned = Math.floor(MAX_TICK / tickSpacing) * tickSpacing;
  return tickLower === minAligned && tickUpper === maxAligned;
}

function calculatePositionAmounts(
  liquidity: bigint,
  sqrtPriceX64: bigint,
  tickLower: number,
  tickUpper: number,
  tickSpacing: number,
): { amount0: bigint; amount1: bigint; isFullRange: boolean } {
  const fullRange = isFullRange(tickLower, tickUpper, tickSpacing);

  if (liquidity === 0n) {
    return { amount0: 0n, amount1: 0n, isFullRange: fullRange };
  }

  // All BigInt math — no BigNumber needed
  const sqrtPriceLower = tickToSqrtPriceX64(tickLower);
  const sqrtPriceUpper = tickToSqrtPriceX64(tickUpper);

  let amount0 = 0n;
  let amount1 = 0n;

  if (sqrtPriceX64 <= sqrtPriceLower) {
    // Current price below range — all token0
    amount0 = (liquidity * Q64_BI * (sqrtPriceUpper - sqrtPriceLower))
      / (sqrtPriceLower * sqrtPriceUpper);
  } else if (sqrtPriceX64 >= sqrtPriceUpper) {
    // Current price above range — all token1
    amount1 = (liquidity * (sqrtPriceUpper - sqrtPriceLower)) / Q64_BI;
  } else {
    // In range
    amount0 = (liquidity * Q64_BI * (sqrtPriceUpper - sqrtPriceX64))
      / (sqrtPriceX64 * sqrtPriceUpper);
    amount1 = (liquidity * (sqrtPriceX64 - sqrtPriceLower)) / Q64_BI;
  }

  return { amount0, amount1, isFullRange: fullRange };
}

// =============================================================================
// SIGNATURE — Ed25519 canonical message + signing
//
// Must byte-match: lp-bonds/src/ed25519.rs::reconstruct_canonical_message
//
// Layout (198 bytes):
//   0-17   "LP_BONDS_SOLANA_V1"       (18)
//  18-49   bond_mint                   (32)
//  50-81   position_mint               (32)
//  82-89   amount0         u64 LE      ( 8)
//  90-97   amount1         u64 LE      ( 8)
//  98-113  liquidity       u128 LE     (16)
// 114-117  tick_lower      i32 LE      ( 4)
// 118-121  tick_upper      i32 LE      ( 4)
// 122-125  tick_current    i32 LE      ( 4)
// 126-133  nonce           u64 LE      ( 8)
// 134-165  sender                      (32)
// 166-197  contract_address            (32)
// =============================================================================

function buildCanonicalMessage(params: {
  bondMint: PublicKey;
  positionMint: PublicKey;
  amount0: bigint;
  amount1: bigint;
  liquidity: bigint;
  tickLower: number;
  tickUpper: number;
  tickCurrent: number;
  nonce: bigint;
  sender: PublicKey;
  contractAddress: PublicKey;
}): Buffer {
  const msg = Buffer.alloc(CANONICAL_MESSAGE_LEN);
  let o = 0;

  Buffer.from(SIGNATURE_DOMAIN, "utf-8").copy(msg, o); o += 18;
  params.bondMint.toBuffer().copy(msg, o); o += 32;
  params.positionMint.toBuffer().copy(msg, o); o += 32;
  msg.writeBigUInt64LE(params.amount0, o); o += 8;
  msg.writeBigUInt64LE(params.amount1, o); o += 8;
  // u128 LE = low u64 then high u64
  const liqLo = params.liquidity & 0xFFFFFFFFFFFFFFFFn;
  const liqHi = params.liquidity >> 64n;
  msg.writeBigUInt64LE(liqLo, o);
  msg.writeBigUInt64LE(liqHi, o + 8);
  o += 16;
  msg.writeInt32LE(params.tickLower, o); o += 4;
  msg.writeInt32LE(params.tickUpper, o); o += 4;
  msg.writeInt32LE(params.tickCurrent, o); o += 4;
  msg.writeBigUInt64LE(params.nonce, o); o += 8;
  params.sender.toBuffer().copy(msg, o); o += 32;
  params.contractAddress.toBuffer().copy(msg, o); o += 32;

  if (o !== CANONICAL_MESSAGE_LEN) {
    throw new Error(`Message length mismatch: expected ${CANONICAL_MESSAGE_LEN}, got ${o}`);
  }
  return msg;
}

function signPositionData(
  bondMint: PublicKey,
  positionMint: PublicKey,
  amount0: bigint,
  amount1: bigint,
  liquidity: bigint,
  tickLower: number,
  tickUpper: number,
  tickCurrent: number,
  nonce: bigint,
  sender: PublicKey,
): string {
  const keypair = getOracleKeypair();
  const contractAddress = new PublicKey(LP_BONDS_PROGRAM_ID);

  const message = buildCanonicalMessage({
    bondMint, positionMint, amount0, amount1, liquidity,
    tickLower, tickUpper, tickCurrent, nonce, sender, contractAddress,
  });

  const signature = nacl.sign.detached(message, keypair.secretKey);
  return Buffer.from(signature).toString("base64");
}

// =============================================================================
// FETCH ON-CHAIN DATA — replaces EVM's positionManager.positions() + pool.slot0()
// =============================================================================

async function fetchPosition(positionMint: PublicKey, whirlpoolProgramId: PublicKey): Promise<PositionState> {
  const [positionPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), positionMint.toBuffer()],
    whirlpoolProgramId,
  );
  const info = await connection.getAccountInfo(positionPda);
  if (!info) throw new Error(`Position not found for mint: ${positionMint.toBase58()}`);
  if (info.data.length < POSITION_ACCOUNT_SIZE) {
    throw new Error(`Invalid position account size: ${info.data.length}`);
  }
  return decodePosition(info.data as Buffer);
}

async function fetchWhirlpool(address: PublicKey): Promise<WhirlpoolState> {
  const info = await connection.getAccountInfo(address);
  if (!info) throw new Error(`Whirlpool not found: ${address.toBase58()}`);
  if (info.data.length < WHIRLPOOL_ACCOUNT_SIZE) {
    throw new Error(`Invalid whirlpool account size: ${info.data.length}`);
  }
  return decodeWhirlpool(info.data as Buffer);
}

async function getTokenDecimals(mint: PublicKey): Promise<number> {
  const info = await connection.getParsedAccountInfo(mint);
  if (!info.value) throw new Error(`Mint not found: ${mint.toBase58()}`);
  const data = info.value.data;
  if ("parsed" in data) return data.parsed.info.decimals;
  throw new Error(`Cannot parse mint: ${mint.toBase58()}`);
}

// =============================================================================
// MAIN CALCULATION — Solana equivalent of EVM calculatePositionAmounts()
//
// EVM:    tokenId → CHAIN_CONFIG lookup → positionManager.positions() → pool.slot0() → calc
// Solana: tokenId → CHAIN_CONFIG lookup → position PDA → whirlpool account → calc
// =============================================================================

function toStr(val: bigint | number): string {
  return val.toString();
}

function bigIntToString(obj: unknown): unknown {
  if (typeof obj === "bigint") return obj.toString();
  if (Array.isArray(obj)) return obj.map(bigIntToString);
  if (obj && typeof obj === "object") {
    const res: Record<string, unknown> = {};
    for (const k in obj as Record<string, unknown>) {
      res[k] = bigIntToString((obj as Record<string, unknown>)[k]);
    }
    return res;
  }
  return obj;
}

async function calculateAmountsForToken(tokenId: string) {
  const config = CHAIN_CONFIG[tokenId];
  if (!config) throw new Error(`Unsupported tokenId: ${tokenId}`);

  const positionMint = new PublicKey(tokenId);
  const whirlpoolProgramId = new PublicKey(config.positionManager);
  const whirlpoolAddress = new PublicKey(config.poolAddress);

  // Fetch on-chain state (equivalent of positionManager.positions() + pool.slot0())
  const [position, whirlpool] = await Promise.all([
    fetchPosition(positionMint, whirlpoolProgramId),
    fetchWhirlpool(whirlpoolAddress),
  ]);

  const liquidity = position.liquidity;

  if (liquidity === 0n) {
    return {
      tokenId,
      amount0: "0",
      amount1: "0",
      hasLiquidity: false,
      liquidity: "0",
    };
  }

  const amounts = calculatePositionAmounts(
    liquidity,
    whirlpool.sqrtPriceX64,
    position.tickLowerIndex,
    position.tickUpperIndex,
    whirlpool.tickSpacing,
  );

  // Fetch token info (same as EVM getTokenInfo)
  const [decimalsA, decimalsB] = await Promise.all([
    getTokenDecimals(whirlpool.tokenMintA),
    getTokenDecimals(whirlpool.tokenMintB),
  ]);

  return {
    tokenId,
    amount0: toStr(amounts.amount0),
    amount1: toStr(amounts.amount1),
    hasLiquidity: true,
    liquidity: toStr(liquidity),
    poolAddress: config.poolAddress,
    tickLower: position.tickLowerIndex,
    tickUpper: position.tickUpperIndex,
    tickCurrent: whirlpool.tickCurrentIndex,
    fee: whirlpool.feeRate,
    positionMint: position.positionMint.toBase58(),
    whirlpool: position.whirlpool.toBase58(),
    token0: {
      address: whirlpool.tokenMintA.toBase58(),
      decimals: decimalsA,
    },
    token1: {
      address: whirlpool.tokenMintB.toBase58(),
      decimals: decimalsB,
    },
  };
}

// =============================================================================
// ENDPOINTS — GET /health + POST /position-info (matching EVM oracle exactly)
// =============================================================================

app.get("/health", (_req, res) => {
  res.status(200).json({ status: "ok", timestamp: new Date().toISOString() });
});

app.post("/position-info", async (req, res) => {
  const { tokenId, nonce, sender, contractAddress } = req.body;
  log("info", "Received /position-info", { tokenId, nonce, ip: req.ip });

  try {
    if (!tokenId) {
      return res.status(400).json({ error: "tokenId is required" });
    }

    const result = await calculateAmountsForToken(tokenId);

    let signature: string | null = null;
    if (sender && req.body.bondMint && process.env.ORACLE_PRIVATE_KEY) {
      const bondMint = new PublicKey(req.body.bondMint);
      const positionMint = new PublicKey(tokenId);

      signature = signPositionData(
        bondMint,
        positionMint,
        BigInt(result.amount0),
        BigInt(result.amount1),
        BigInt(result.liquidity),
        result.tickLower ?? 0,
        result.tickUpper ?? 0,
        result.tickCurrent ?? 0,
        BigInt(nonce || 0),
        new PublicKey(sender),
      );
    }

    res.setHeader("Content-Type", "application/json");
    const response = bigIntToString(result) as Record<string, unknown>;
    response.signature = signature;
    res.json(response);
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : "Unknown error";
    log("error", "Error in /position-info", { error: message });
    res.status(500).json({ error: message });
  }
});

// 404 handler
app.use((_req, res) => {
  res.status(404).json({ error: "Endpoint not found" });
});

// =============================================================================
// START SERVER
// =============================================================================

const PORT = parseInt(process.env.PORT || "3000", 10);
app.listen(PORT, () => {
  log("info", `Solana LP Bond oracle running on port ${PORT}`);
  log("info", `RPC: ${RPC_URL}`);
  log("info", `Program: ${LP_BONDS_PROGRAM_ID}`);
  log("info", `Pools configured: ${Object.keys(CHAIN_CONFIG).length}`);
});

export default app;
