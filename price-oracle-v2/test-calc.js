// Test the full calculateAmountsForToken flow
require("dotenv").config();
const { Connection, PublicKey } = require("@solana/web3.js");
const BigNumber = require("bignumber.js");

BigNumber.config({ DECIMAL_PLACES: 40, ROUNDING_MODE: BigNumber.ROUND_DOWN });

const conn = new Connection("https://api.devnet.solana.com", {commitment: "confirmed"});
const POSITION_ACCOUNT_SIZE = 216;
const WHIRLPOOL_ACCOUNT_SIZE = 653;

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

function isFullRange(tickLower, tickUpper, tickSpacing) {
  var minAligned = Math.ceil(MIN_TICK / tickSpacing) * tickSpacing;
  var maxAligned = Math.floor(MAX_TICK / tickSpacing) * tickSpacing;
  return tickLower === minAligned && tickUpper === maxAligned;
}

function calculatePositionAmounts(liquidity, sqrtPriceX64, tickLower, tickUpper, tickSpacing) {
  if (liquidity === 0n) {
    return { amount0: 0n, amount1: 0n, isFullRange: isFullRange(tickLower, tickUpper, tickSpacing) };
  }
  var L = new BigNumber(liquidity.toString());
  var sqrtPriceCurrent = new BigNumber(sqrtPriceX64.toString());
  var sqrtPriceLower = tickToSqrtPriceX64(tickLower);
  var sqrtPriceUpper = tickToSqrtPriceX64(tickUpper);

  var amount0BN = new BigNumber(0);
  var amount1BN = new BigNumber(0);

  if (sqrtPriceCurrent.lte(sqrtPriceLower)) {
    amount0BN = L.times(Q64).times(sqrtPriceUpper.minus(sqrtPriceLower))
      .div(sqrtPriceLower.times(sqrtPriceUpper)).integerValue(BigNumber.ROUND_DOWN);
  } else if (sqrtPriceCurrent.gte(sqrtPriceUpper)) {
    amount1BN = L.times(sqrtPriceUpper.minus(sqrtPriceLower))
      .div(Q64).integerValue(BigNumber.ROUND_DOWN);
  } else {
    amount0BN = L.times(Q64).times(sqrtPriceUpper.minus(sqrtPriceCurrent))
      .div(sqrtPriceCurrent.times(sqrtPriceUpper)).integerValue(BigNumber.ROUND_DOWN);
    amount1BN = L.times(sqrtPriceCurrent.minus(sqrtPriceLower))
      .div(Q64).integerValue(BigNumber.ROUND_DOWN);
  }

  return {
    amount0: BigInt(amount0BN.toFixed(0)),
    amount1: BigInt(amount1BN.toFixed(0)),
    isFullRange: isFullRange(tickLower, tickUpper, tickSpacing),
  };
}

async function main() {
  var positionMint = new PublicKey("GdcbPDTGxidg2Q5ACYhv7Q5q3AoG1t5FKD83HUEqC6dv");
  var whirlpoolProgramId = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
  var whirlpoolAddress = new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN");

  console.log("Fetching position...");
  var [positionPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), positionMint.toBuffer()],
    whirlpoolProgramId,
  );
  var [posInfo, wpInfo] = await Promise.all([
    conn.getAccountInfo(positionPda),
    conn.getAccountInfo(whirlpoolAddress),
  ]);

  if (!posInfo) { console.log("Position not found"); return; }
  if (!wpInfo) { console.log("Whirlpool not found"); return; }

  console.log("Decoding position...");
  var pd = posInfo.data;
  var liquidity = readU128(pd, 72);
  var tickLower = readI32(pd, 88);
  var tickUpper = readI32(pd, 92);
  console.log("  liquidity:", liquidity.toString(), "ticks:", tickLower, tickUpper);

  console.log("Decoding whirlpool...");
  var wd = wpInfo.data;
  var tickSpacing = readU16(wd, 8 + 33);
  var sqrtPriceX64 = readU128(wd, 8 + 49);
  var tickCurrent = readI32(wd, 8 + 65);
  console.log("  sqrtPriceX64:", sqrtPriceX64.toString(), "tickCurrent:", tickCurrent, "tickSpacing:", tickSpacing);

  console.log("Calculating amounts...");
  var result = calculatePositionAmounts(liquidity, sqrtPriceX64, tickLower, tickUpper, tickSpacing);
  console.log("  amount0:", result.amount0.toString());
  console.log("  amount1:", result.amount1.toString());
  console.log("  isFullRange:", result.isFullRange);

  // Compare with the other oracle's output
  console.log("\nExpected from other oracle: amount0=1000000007, amount1=1000099346394");
}

main().catch(function(e) { console.error("Error:", e); });
