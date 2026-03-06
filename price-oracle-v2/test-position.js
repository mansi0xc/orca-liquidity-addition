const { Connection, PublicKey } = require("@solana/web3.js");
const conn = new Connection("https://api.devnet.solana.com", {commitment: "confirmed"});
const positionMint = new PublicKey("GdcbPDTGxidg2Q5ACYhv7Q5q3AoG1t5FKD83HUEqC6dv");
const whirlpoolProgramId = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");
const [positionPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("position"), positionMint.toBuffer()],
  whirlpoolProgramId,
);
console.log("Position PDA:", positionPda.toBase58());
conn.getAccountInfo(positionPda).then(function(info) {
  if (!info) { console.log("NOT FOUND"); return; }
  console.log("Account found, size:", info.data.length, "owner:", info.owner.toBase58());
  var data = info.data;
  var whirlpool = new PublicKey(data.subarray(8, 40));
  var posMint = new PublicKey(data.subarray(40, 72));
  console.log("Whirlpool:", whirlpool.toBase58());
  console.log("Position Mint:", posMint.toBase58());
  var lo = data.readBigUInt64LE(72);
  var hi = data.readBigUInt64LE(80);
  var liq = (hi << 64n) | lo;
  console.log("Liquidity:", liq.toString());
  var tickLower = data.readInt32LE(88);
  var tickUpper = data.readInt32LE(92);
  console.log("Tick Lower:", tickLower, "Tick Upper:", tickUpper);
}).catch(function(e) { console.error("Error:", e.message); });
