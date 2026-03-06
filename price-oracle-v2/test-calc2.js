// Test with correct offsets matching the oracle's decodeWhirlpool
require("dotenv").config();
const { Connection, PublicKey } = require("@solana/web3.js");
const BigNumber = require("bignumber.js");

BigNumber.config({ DECIMAL_PLACES: 40, ROUNDING_MODE: BigNumber.ROUND_DOWN });

const conn = new Connection("https://api.devnet.solana.com", {commitment: "confirmed"});

function readU16(buf, offset) { return buf.readUInt16LE(offset); }
function readI32(buf, offset) { return buf.readInt32LE(offset); }
function readU128(buf, offset) {
  var lo = buf.readBigUInt64LE(offset);
  var hi = buf.readBigUInt64LE(offset + 8);
  return (hi << 64n) | lo;
}

var Q64 = new BigNumber(2).pow(64);
var MIN_TICK = -443636;
var MAX_TICK = 443636;

function tickToSqrtPriceX64(tick) {
  var sqrtPrice = new BigNumber(1.0001).pow(tick / 2);
  return sqrtPrice.times(Q64).integerValue(BigNumber.ROUND_DOWN);
}

function decodeWhirlpool(data) {
  var o = 8;
  o += 32; // whirlpools_config
  o += 1;  // whirlpool_bump
  var tickSpacing = readU16(data, o); o += 2;
  o += 2;  // tick_spacing_seed
  var feeRate = readU16(data, o); o += 2;
  o += 2;  // protocol_fee_rate
  var liquidity = readU128(data, o); o += 16;
  var sqrtPriceX64 = readU128(data, o); o += 16;
  var tickCurrentIndex = readI32(data, o); o += 4;
  o += 8; // protocol_fee_owed_a
  o += 8; // protocol_fee_owed_b
  var tokenMintA = new PublicKey(data.subarray(o, o + 32)); o += 32;
  o += 32; // token_vault_a
  o += 16; // fee_growth_global_a
  var tokenMintB = new PublicKey(data.subarray(o, o + 32));
  return { sqrtPriceX64, tickCurrentIndex, tickSpacing, liquidity, tokenMintA, tokenMintB, feeRate };
}

async function main() {
  var positionMint = new PublicKey("GdcbPDTGxidg2Q5ACYhv7Q5q3AoG1t5FKD83HUEqC6dv");
  var whirlpoolProgramId = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
  var whirlpoolAddress = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");

  console.log("Fetching position + whirlpool...");
  var [positionPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), positionMint.toBuffer()],
    whirlpoolProgramId,
  );
  var [posInfo, wpInfo] = await Promise.all([
    conn.getAccountInfo(positionPda),
    conn.getAccountInfo(whirlpoolAddress),
  ]);

  console.log("Decoding position...");
  var pd = posInfo.data;
  var liquidity = readU128(pd, 72);
  var tickLower = readI32(pd, 88);
  var tickUpper = readI32(pd, 92);
  console.log("  liquidity:", liquidity.toString(), "ticks:", tickLower, tickUpper);

  console.log("Decoding whirlpool (using oracle offsets)...");
  var wp = decodeWhirlpool(wpInfo.data);
  console.log("  sqrtPriceX64:", wp.sqrtPriceX64.toString());
  console.log("  tickCurrent:", wp.tickCurrentIndex);
  console.log("  tickSpacing:", wp.tickSpacing);
  console.log("  tokenMintA:", wp.tokenMintA.toBase58());
  console.log("  tokenMintB:", wp.tokenMintB.toBase58());

  console.log("\nCalculating sqrtPriceLower and sqrtPriceUpper...");
  console.time("tickToSqrtPriceX64-lower");
  var sqrtPriceLower = tickToSqrtPriceX64(tickLower);
  console.timeEnd("tickToSqrtPriceX64-lower");
  console.log("  sqrtPriceLower:", sqrtPriceLower.toFixed(0));

  console.time("tickToSqrtPriceX64-upper");
  var sqrtPriceUpper = tickToSqrtPriceX64(tickUpper);
  console.timeEnd("tickToSqrtPriceX64-upper");
  console.log("  sqrtPriceUpper:", sqrtPriceUpper.toFixed(0));

  console.log("\nCalculating amounts...");
  var L = new BigNumber(liquidity.toString());
  var sqrtPriceCurrent = new BigNumber(wp.sqrtPriceX64.toString());

  console.log("  Current vs bounds:", sqrtPriceCurrent.toFixed(0), "between", sqrtPriceLower.toFixed(0), "and", sqrtPriceUpper.toFixed(0));

  console.time("amount-calc");
  var amount0BN, amount1BN;
  if (sqrtPriceCurrent.lte(sqrtPriceLower)) {
    console.log("  Case: below range (all token0)");
    amount0BN = L.times(Q64).times(sqrtPriceUpper.minus(sqrtPriceLower))
      .div(sqrtPriceLower.times(sqrtPriceUpper)).integerValue(BigNumber.ROUND_DOWN);
    amount1BN = new BigNumber(0);
  } else if (sqrtPriceCurrent.gte(sqrtPriceUpper)) {
    console.log("  Case: above range (all token1)");
    amount0BN = new BigNumber(0);
    amount1BN = L.times(sqrtPriceUpper.minus(sqrtPriceLower))
      .div(Q64).integerValue(BigNumber.ROUND_DOWN);
  } else {
    console.log("  Case: in range");
    amount0BN = L.times(Q64).times(sqrtPriceUpper.minus(sqrtPriceCurrent))
      .div(sqrtPriceCurrent.times(sqrtPriceUpper)).integerValue(BigNumber.ROUND_DOWN);
    amount1BN = L.times(sqrtPriceCurrent.minus(sqrtPriceLower))
      .div(Q64).integerValue(BigNumber.ROUND_DOWN);
  }
  console.timeEnd("amount-calc");
  console.log("  amount0:", amount0BN.toFixed(0));
  console.log("  amount1:", amount1BN.toFixed(0));

  console.log("\nExpected: amount0=1000000007, amount1=1000099346394");

  // Test getTokenDecimals
  console.log("\nFetching token decimals...");
  console.time("decimals");
  var [mintAInfo, mintBInfo] = await Promise.all([
    conn.getParsedAccountInfo(wp.tokenMintA),
    conn.getParsedAccountInfo(wp.tokenMintB),
  ]);
  console.timeEnd("decimals");
  console.log("  tokenA decimals:", mintAInfo.value.data.parsed.info.decimals);
  console.log("  tokenB decimals:", mintBInfo.value.data.parsed.info.decimals);

  console.log("\nDONE");
}

main().catch(function(e) { console.error("Error:", e); });
