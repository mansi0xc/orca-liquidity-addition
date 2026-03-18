/**
 * User Testing Script for LP Bonds Protocol (Phase 4)
 *
 * Simulates a real user flow with a SEPARATE wallet (not admin):
 *   1. Generate test wallet keypair (or load existing)
 *   2. Print address and STOP for manual funding
 *   3. After funding, mint a Level 1 bond
 *   4. Evolve L1 -> L2 -> L3 -> L4
 *   5. Verify NFTs, positions, token balances, evolution state
 *
 * The admin wallet (oracle authority) is needed to sign evolution messages.
 *
 * Usage:
 *   # Step 1 - Generate wallet and print address:
 *   npx ts-node --project tsconfig.scripts.json scripts/user-test.ts generate
 *
 *   # Step 2 - Fund the printed address with SOL + tokens on devnet
 *
 *   # Step 3 - Run the full test:
 *   npx ts-node --project tsconfig.scripts.json scripts/user-test.ts run
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  ComputeBudgetProgram,
  Connection,
  SystemProgram,
  Transaction,
  TransactionMessage,
  VersionedTransaction,
  Ed25519Program,
  AddressLookupTableProgram,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import {
  NATIVE_MINT,
  getAssociatedTokenAddress,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import * as fs from "fs";
import * as os from "os";
import * as nacl from "tweetnacl";

// =============================================================================
// CONSTANTS
// =============================================================================

const WHIRLPOOL_PROGRAM_ID = new PublicKey("whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc");

const TEST_WALLET_PATH = "./keys/test-user-wallet.json";

const CONFIG_SEED = Buffer.from("config");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const POSITION_CUSTODY_SEED = Buffer.from("position_custody");
const ORACLE_CONFIG_SEED = Buffer.from("oracle_config");
const NONCE_SEED = Buffer.from("nonce");
const EVOLUTION_CONFIG_SEED = Buffer.from("evolution_config");
const LEVEL_CONFIG_SEED = Buffer.from("level_config");
const LAYER_TOKEN_AUTHORITY_SEED = Buffer.from("layer_token_authority");
const EVOLUTION_NONCE_SEED = Buffer.from("evolution_nonce");

const EVOLUTION_SIGNATURE_DOMAIN = Buffer.from("LP_BONDS_EVOLVE_V1");

const TICK_ARRAY_SIZE = 88;

// =============================================================================
// HELPERS
// =============================================================================

function loadKeypair(path: string): Keypair {
  const resolved = path.replace("~", os.homedir());
  const data = JSON.parse(fs.readFileSync(resolved, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(data));
}

function saveKeypair(kp: Keypair, path: string): void {
  if (!fs.existsSync("./keys")) fs.mkdirSync("./keys", { recursive: true });
  fs.writeFileSync(path, JSON.stringify(Array.from(kp.secretKey)));
}

function explorerTx(sig: string): string {
  return `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
}

function explorerAddr(addr: string): string {
  return `https://explorer.solana.com/address/${addr}?cluster=devnet`;
}

function parseWhirlpoolVaults(data: Buffer): { vaultA: PublicKey; vaultB: PublicKey; tickSpacing: number; currentTick: number } {
  const tickSpacing = data.readUInt16LE(8 + 32 + 1);
  const currentTick = data.readInt32LE(8 + 32 + 1 + 2 + 2 + 2 + 2 + 16 + 16);
  const vaultA = new PublicKey(data.subarray(133, 165));
  const vaultB = new PublicKey(data.subarray(213, 245));
  return { vaultA, vaultB, tickSpacing, currentTick };
}

function deriveTickArrayPda(whirlpool: PublicKey, tickIndex: number, tickSpacing: number): PublicKey {
  const ticksInArray = tickSpacing * TICK_ARRAY_SIZE;
  const startTickIndex = Math.floor(tickIndex / ticksInArray) * ticksInArray;
  const [pda] = PublicKey.findProgramAddressSync(
    [Buffer.from("tick_array"), whirlpool.toBuffer(), Buffer.from(startTickIndex.toString())],
    WHIRLPOOL_PROGRAM_ID,
  );
  return pda;
}

function buildEvolutionCanonicalMessage(params: {
  sourceBondMint: PublicKey;
  targetLevel: number;
  amountA: BN;
  amountB: BN;
  liquidity: BN;
  nonce: BN;
  sender: PublicKey;
  contractAddress: PublicKey;
}): Buffer {
  const buf = Buffer.alloc(155);
  let offset = 0;

  EVOLUTION_SIGNATURE_DOMAIN.copy(buf, offset);
  offset += 18;

  buf.set(params.sourceBondMint.toBytes(), offset);
  offset += 32;

  buf.writeUInt8(params.targetLevel, offset);
  offset += 1;

  buf.writeBigUInt64LE(BigInt(params.amountA.toString()), offset);
  offset += 8;

  buf.writeBigUInt64LE(BigInt(params.amountB.toString()), offset);
  offset += 8;

  const liq = BigInt(params.liquidity.toString());
  buf.writeBigUInt64LE(liq & BigInt("0xFFFFFFFFFFFFFFFF"), offset);
  offset += 8;
  buf.writeBigUInt64LE(liq >> BigInt(64), offset);
  offset += 8;

  buf.writeBigUInt64LE(BigInt(params.nonce.toString()), offset);
  offset += 8;

  buf.set(params.sender.toBytes(), offset);
  offset += 32;

  buf.set(params.contractAddress.toBytes(), offset);

  return buf;
}

// =============================================================================
// STEP: GENERATE WALLET
// =============================================================================

async function generateWallet() {
  console.log("=".repeat(70));
  console.log("LP BONDS - USER TEST WALLET GENERATION");
  console.log("=".repeat(70));

  let kp: Keypair;
  if (fs.existsSync(TEST_WALLET_PATH)) {
    kp = loadKeypair(TEST_WALLET_PATH);
    console.log("\nExisting test wallet loaded:");
  } else {
    kp = Keypair.generate();
    saveKeypair(kp, TEST_WALLET_PATH);
    console.log("\nNew test wallet generated:");
  }

  console.log(`  Address: ${kp.publicKey.toBase58()}`);
  console.log(`  Path:    ${TEST_WALLET_PATH}`);
  console.log(`  Explorer: ${explorerAddr(kp.publicKey.toBase58())}`);

  console.log("\n" + "=".repeat(70));
  console.log("PLEASE FUND THIS WALLET BEFORE RUNNING THE TEST");
  console.log("=".repeat(70));
  console.log("\nRequired funding:");
  console.log("  - At least 2 SOL for transaction fees + liquidity deposits");
  console.log("  - Token B for Level 1 whirlpool (4qbX8...) if two-sided range");
  console.log("  - Token A (4qbX8...) for evolution levels 2-4");
  console.log("  NOTE: Layer tokens are minted internally by the protocol (EVM parity)");
  console.log("\nAfter funding, run:");
  console.log("  npx ts-node --project tsconfig.scripts.json scripts/user-test.ts run");
  console.log("=".repeat(70));
}

// =============================================================================
// STEP: RUN FULL TEST
// =============================================================================

async function runTest() {
  console.log("=".repeat(70));
  console.log("LP BONDS - USER TEST FLOW");
  console.log("=".repeat(70));

  // --- Load wallets ---
  if (!fs.existsSync(TEST_WALLET_PATH)) {
    throw new Error("Test wallet not found. Run 'generate' first.");
  }

  const testUser = loadKeypair(TEST_WALLET_PATH);
  const adminKp = loadKeypair(process.env.ANCHOR_WALLET || "~/.config/solana/id.json");

  const rpcUrl = process.env.ANCHOR_PROVIDER_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");
  const userWallet = new anchor.Wallet(testUser);
  const provider = new anchor.AnchorProvider(connection, userWallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  console.log("\nTest user:  ", testUser.publicKey.toBase58());
  console.log("Admin/Oracle:", adminKp.publicKey.toBase58());

  const balance = await connection.getBalance(testUser.publicKey);
  console.log("User balance:", balance / LAMPORTS_PER_SOL, "SOL");

  if (balance < 0.5 * LAMPORTS_PER_SOL) {
    throw new Error("Test wallet needs at least 0.5 SOL. Please fund it first.");
  }

  // --- Load programs ---
  const lpBondsIdl = JSON.parse(fs.readFileSync("./target/idl/lp_bonds.json", "utf-8"));
  const evolutionIdl = JSON.parse(fs.readFileSync("./target/idl/lp_bonds_evolution.json", "utf-8"));

  const lpBondsProgram = new Program(lpBondsIdl, provider);
  const evolutionProgram = new Program(evolutionIdl, provider);

  const LP_BONDS_PROGRAM_ID = lpBondsProgram.programId;
  const EVOLUTION_PROGRAM_ID = evolutionProgram.programId;

  console.log("LP Bonds:   ", LP_BONDS_PROGRAM_ID.toBase58());
  console.log("Evolution:  ", EVOLUTION_PROGRAM_ID.toBase58());

  // --- Derive shared PDAs ---
  const [configPda] = PublicKey.findProgramAddressSync([CONFIG_SEED], LP_BONDS_PROGRAM_ID);
  const [bondAuthorityPda] = PublicKey.findProgramAddressSync([BOND_AUTHORITY_SEED], LP_BONDS_PROGRAM_ID);
  const [evolutionConfigPda] = PublicKey.findProgramAddressSync([EVOLUTION_CONFIG_SEED], EVOLUTION_PROGRAM_ID);
  const [layerTokenAuthorityPda] = PublicKey.findProgramAddressSync([LAYER_TOKEN_AUTHORITY_SEED], EVOLUTION_PROGRAM_ID);
  const [evoBondAuthorityPda] = PublicKey.findProgramAddressSync([BOND_AUTHORITY_SEED], EVOLUTION_PROGRAM_ID);

  // Fetch protocol config
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const lpAccounts = lpBondsProgram.account as any;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const evoAccounts = evolutionProgram.account as any;

  const protocolConfig = await lpAccounts.protocolConfig.fetch(configPda);
  const L1_WHIRLPOOL = protocolConfig.allowlistedWhirlpool as PublicKey;
  const L1_TOKEN_B = protocolConfig.tokenMintB as PublicKey;

  console.log("\nL1 Whirlpool:", L1_WHIRLPOOL.toBase58());
  console.log("L1 Token B:  ", L1_TOKEN_B.toBase58());

  // =========================================================================
  // TEST 1: Initialize User Nonce Accounts
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("TEST 1: Initialize Nonce Accounts");
  console.log("-".repeat(70));

  const [baseNoncePda] = PublicKey.findProgramAddressSync(
    [NONCE_SEED, testUser.publicKey.toBuffer()],
    LP_BONDS_PROGRAM_ID,
  );

  const [evoNoncePda] = PublicKey.findProgramAddressSync(
    [EVOLUTION_NONCE_SEED, testUser.publicKey.toBuffer()],
    EVOLUTION_PROGRAM_ID,
  );

  if (await connection.getAccountInfo(baseNoncePda)) {
    console.log("  Base nonce already initialized.");
  } else {
    const sig = await lpBondsProgram.methods
      .initializeNonce()
      .accountsPartial({
        user: testUser.publicKey,
        nonceAccount: baseNoncePda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 })])
      .rpc();
    await connection.confirmTransaction(sig, "confirmed");
    console.log("  Base nonce initialized:", explorerTx(sig));
  }

  if (await connection.getAccountInfo(evoNoncePda)) {
    console.log("  Evolution nonce already initialized.");
  } else {
    const sig = await evolutionProgram.methods
      .initializeEvolutionNonce()
      .accountsPartial({
        user: testUser.publicKey,
        evolutionNonce: evoNoncePda,
        systemProgram: SystemProgram.programId,
      })
      .preInstructions([ComputeBudgetProgram.setComputeUnitLimit({ units: 200_000 })])
      .rpc();
    await connection.confirmTransaction(sig, "confirmed");
    console.log("  Evolution nonce initialized:", explorerTx(sig));
  }

  // =========================================================================
  // TEST 2: Mint Level 1 Bond
  // =========================================================================

  console.log("\n" + "-".repeat(70));
  console.log("TEST 2: Mint Level 1 Bond");
  console.log("-".repeat(70));

  const whirlpoolInfo = await connection.getAccountInfo(L1_WHIRLPOOL);
  if (!whirlpoolInfo) throw new Error("L1 Whirlpool not found");
  const wpData = parseWhirlpoolVaults(whirlpoolInfo.data as Buffer);

  console.log("  Whirlpool vault A:", wpData.vaultA.toBase58());
  console.log("  Whirlpool vault B:", wpData.vaultB.toBase58());
  console.log("  Tick spacing:     ", wpData.tickSpacing);
  console.log("  Current tick:     ", wpData.currentTick);

  const bondMint = Keypair.generate();
  const positionMint = Keypair.generate();
  const userWsolAccount = Keypair.generate();

  const tickRange = wpData.tickSpacing * 10;
  const tickLower = Math.floor((wpData.currentTick - tickRange) / wpData.tickSpacing) * wpData.tickSpacing;
  const tickUpper = Math.ceil((wpData.currentTick + tickRange) / wpData.tickSpacing) * wpData.tickSpacing;

  console.log("  Tick range:       ", tickLower, "to", tickUpper);

  const tickArrayLower = deriveTickArrayPda(L1_WHIRLPOOL, tickLower, wpData.tickSpacing);
  const tickArrayUpper = deriveTickArrayPda(L1_WHIRLPOOL, tickUpper, wpData.tickSpacing);

  const taLowerExists = await connection.getAccountInfo(tickArrayLower);
  const taUpperExists = await connection.getAccountInfo(tickArrayUpper);
  if (!taLowerExists || !taUpperExists) {
    throw new Error("Tick arrays not initialized for selected range. Try a different range.");
  }

  const [positionCustodyPda] = PublicKey.findProgramAddressSync(
    [POSITION_CUSTODY_SEED, bondMint.publicKey.toBuffer()],
    LP_BONDS_PROGRAM_ID,
  );

  const [whirlpoolPositionPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), positionMint.publicKey.toBuffer()],
    WHIRLPOOL_PROGRAM_ID,
  );

  const userBondAccount = await getAssociatedTokenAddress(bondMint.publicKey, testUser.publicKey);
  const userTokenBAccount = await getAssociatedTokenAddress(L1_TOKEN_B, testUser.publicKey);
  const positionTokenAccount = await getAssociatedTokenAddress(positionMint.publicKey, testUser.publicKey);
  const custodyPositionTokenAccount = await getAssociatedTokenAddress(
    positionMint.publicKey,
    positionCustodyPda,
    true,
  );

  const solAmount = new BN(0.01 * LAMPORTS_PER_SOL);
  const liquidityAmount = new BN(1000);
  const tokenMaxA = solAmount;
  const tokenMaxB = new BN(100_000);

  console.log("  Bond mint:        ", bondMint.publicKey.toBase58());
  console.log("  SOL amount:       ", solAmount.toString(), "lamports");

  const mintSig = await lpBondsProgram.methods
    .addLiquidityAndMintBond(
      tickLower,
      tickUpper,
      liquidityAmount,
      tokenMaxA,
      tokenMaxB,
      solAmount,
    )
    .accountsPartial({
      user: testUser.publicKey,
      wsolMint: NATIVE_MINT,
      tokenMintB: L1_TOKEN_B,
      bondAuthority: bondAuthorityPda,
      bondMint: bondMint.publicKey,
      userWsolAccount: userWsolAccount.publicKey,
      userTokenBAccount,
      userBondAccount,
      config: configPda,
      positionCustody: positionCustodyPda,
      positionMint: positionMint.publicKey,
      whirlpoolPosition: whirlpoolPositionPda,
      positionTokenAccount,
      custodyPositionTokenAccount,
      whirlpool: L1_WHIRLPOOL,
      tokenVaultA: wpData.vaultA,
      tokenVaultB: wpData.vaultB,
      tickArrayLower,
      tickArrayUpper,
      whirlpoolProgram: WHIRLPOOL_PROGRAM_ID,
    })
    .preInstructions([
      ComputeBudgetProgram.setComputeUnitLimit({ units: 600_000 }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 5_000 }),
    ])
    .signers([bondMint, positionMint, userWsolAccount])
    .rpc();

  await connection.confirmTransaction(mintSig, "confirmed");
  console.log("  Level 1 bond minted:", explorerTx(mintSig));
  console.log("  Bond NFT mint:      ", bondMint.publicKey.toBase58());

  // Verify bond
  const bondAccountInfo = await connection.getTokenAccountBalance(userBondAccount);
  console.log("  Bond balance:       ", bondAccountInfo.value.uiAmount);

  const custodyData = await lpAccounts.positionCustody.fetch(positionCustodyPda);
  console.log("  Custody level:      ", custodyData.level);
  console.log("  Custody depositor:  ", custodyData.depositor.toBase58());

  // =========================================================================
  // TEST 3-5: Evolve Bond L1 -> L2 -> L3 -> L4
  // =========================================================================

  let currentBondMint = bondMint.publicKey;
  let currentBondAccount = userBondAccount;
  let currentCustodyOwner = LP_BONDS_PROGRAM_ID;

  for (const targetLevel of [2, 3, 4]) {
    console.log("\n" + "-".repeat(70));
    console.log(`TEST ${targetLevel + 1}: Evolve to Level ${targetLevel}`);
    console.log("-".repeat(70));

    const [levelConfigPda] = PublicKey.findProgramAddressSync(
      [LEVEL_CONFIG_SEED, Buffer.from([targetLevel])],
      EVOLUTION_PROGRAM_ID,
    );

    const levelConfig = await evoAccounts.levelConfig.fetch(levelConfigPda);
    const levelWhirlpool = levelConfig.whirlpool as PublicKey;
    const levelTokenA = levelConfig.tokenMintA as PublicKey;
    const levelLayerToken = levelConfig.layerTokenMint as PublicKey;
    const requiredAmountA = levelConfig.requiredAmountA as BN;
    const requiredAmountB = levelConfig.requiredAmountB as BN;

    console.log("  Whirlpool:    ", levelWhirlpool.toBase58());
    console.log("  Token A:      ", levelTokenA.toBase58());
    console.log("  Layer Token:  ", levelLayerToken.toBase58());
    console.log("  Required A:   ", requiredAmountA.toString());
    console.log("  Required B:   ", requiredAmountB.toString());

    const wpInfo = await connection.getAccountInfo(levelWhirlpool);
    if (!wpInfo) throw new Error(`Whirlpool for level ${targetLevel} not found`);
    const lvlWpData = parseWhirlpoolVaults(wpInfo.data as Buffer);

    const lvlTickRange = lvlWpData.tickSpacing * 10;
    const lvlTickLower = Math.floor((lvlWpData.currentTick - lvlTickRange) / lvlWpData.tickSpacing) * lvlWpData.tickSpacing;
    const lvlTickUpper = Math.ceil((lvlWpData.currentTick + lvlTickRange) / lvlWpData.tickSpacing) * lvlWpData.tickSpacing;

    const lvlTickArrayLower = deriveTickArrayPda(levelWhirlpool, lvlTickLower, lvlWpData.tickSpacing);
    const lvlTickArrayUpper = deriveTickArrayPda(levelWhirlpool, lvlTickUpper, lvlWpData.tickSpacing);

    console.log("  Tick lower:    ", lvlTickLower);
    console.log("  Tick upper:    ", lvlTickUpper);
    console.log("  Tick array (L):", lvlTickArrayLower.toBase58());
    console.log("  Tick array (U):", lvlTickArrayUpper.toBase58());

    const taLExists = await connection.getAccountInfo(lvlTickArrayLower);
    const taUExists = await connection.getAccountInfo(lvlTickArrayUpper);
    if (!taLExists || !taUExists) {
      throw new Error(`Tick arrays not initialized for level ${targetLevel} whirlpool. Cannot add liquidity.`);
    }
    console.log("  Tick arrays:    VERIFIED");

    // Generate new target bond + position keypairs
    const targetBondMint = Keypair.generate();
    const targetPositionMint = Keypair.generate();

    // Nonce: fetch current and increment
    const evoNonceData = await evoAccounts.evolutionNonce.fetch(evoNoncePda);
    const newNonce = new BN((evoNonceData.currentNonce as BN).toNumber() + 1);

    // Liquidity must be small enough that the Whirlpool math doesn't overflow
    // and token amounts needed stay within token_max bounds.
    const liquidityAmount = new BN("1000000");

    // Build oracle signature
    const evolutionMessage = buildEvolutionCanonicalMessage({
      sourceBondMint: currentBondMint,
      targetLevel,
      amountA: requiredAmountA,
      amountB: requiredAmountB,
      liquidity: liquidityAmount,
      nonce: newNonce,
      sender: testUser.publicKey,
      contractAddress: EVOLUTION_PROGRAM_ID,
    });

    const signature = nacl.sign.detached(evolutionMessage, adminKp.secretKey);

    // Derive accounts
    const sourceCustodyPda = PublicKey.findProgramAddressSync(
      [POSITION_CUSTODY_SEED, currentBondMint.toBuffer()],
      currentCustodyOwner,
    )[0];

    const [targetPositionCustodyPda] = PublicKey.findProgramAddressSync(
      [POSITION_CUSTODY_SEED, targetBondMint.publicKey.toBuffer()],
      EVOLUTION_PROGRAM_ID,
    );

    const [evolutionRecordPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("evolution_record"), currentBondMint.toBuffer()],
      EVOLUTION_PROGRAM_ID,
    );

    const [targetWhirlpoolPositionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("position"), targetPositionMint.publicKey.toBuffer()],
      WHIRLPOOL_PROGRAM_ID,
    );

    const userTargetBondAccount = await getAssociatedTokenAddress(targetBondMint.publicKey, testUser.publicKey);
    const userTokenAAccount = await getAssociatedTokenAddress(levelTokenA, testUser.publicKey);

    const programTokenAAccount = await getAssociatedTokenAddress(levelTokenA, layerTokenAuthorityPda, true);
    const programTokenBAccount = await getAssociatedTokenAddress(levelLayerToken, layerTokenAuthorityPda, true);

    const evoConfig = await evoAccounts.evolutionConfig.fetch(evolutionConfigPda);
    const treasuryTokenAccount = await getAssociatedTokenAddress(levelTokenA, evoConfig.treasury as PublicKey, true);

    const targetPositionTokenAccount = await getAssociatedTokenAddress(
      targetPositionMint.publicKey,
      testUser.publicKey,
    );
    const targetCustodyPositionTokenAccount = await getAssociatedTokenAddress(
      targetPositionMint.publicKey,
      layerTokenAuthorityPda,
      true,
    );

    // Ensure user token A account exists
    try {
      await getOrCreateAssociatedTokenAccount(connection, testUser, levelTokenA, testUser.publicKey);
    } catch (_e) { /* may already exist */ }

    // Ensure treasury token account exists
    try {
      await getOrCreateAssociatedTokenAccount(connection, testUser, levelTokenA, evoConfig.treasury as PublicKey, true);
    } catch (_e) { /* may already exist */ }

    // Build Ed25519 instruction
    const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
      publicKey: adminKp.publicKey.toBytes(),
      message: evolutionMessage,
      signature,
    });

    // Build evolve instruction
    const evolveIx = await evolutionProgram.methods
      .evolveBond(
        targetLevel,
        requiredAmountA,
        requiredAmountB,
        liquidityAmount, // small liquidity for Whirlpool math to stay within bounds
        requiredAmountA, // token_max_a
        requiredAmountB, // token_max_b
        lvlTickLower,
        lvlTickUpper,
        newNonce,
      )
      .accountsPartial({
        user: testUser.publicKey,
        evolutionConfig: evolutionConfigPda,
        levelConfig: levelConfigPda,
        evolutionNonce: evoNoncePda,
        sourceBondMint: currentBondMint,
        userSourceBondAccount: currentBondAccount,
        sourceCustody: sourceCustodyPda,
        bondAuthority: evoBondAuthorityPda,
        targetBondMint: targetBondMint.publicKey,
        userTargetBondAccount,
        positionCustody: targetPositionCustodyPda,
        evolutionRecord: evolutionRecordPda,
        tokenMintA: levelTokenA,
        layerTokenMint: levelLayerToken,
        layerTokenAuthority: layerTokenAuthorityPda,
        userTokenAAccount,
        programTokenAAccount,
        programTokenBAccount,
        treasuryTokenAccount,
        whirlpool: levelWhirlpool,
        positionMint: targetPositionMint.publicKey,
        whirlpoolPosition: targetWhirlpoolPositionPda,
        positionTokenAccount: targetPositionTokenAccount,
        custodyPositionTokenAccount: targetCustodyPositionTokenAccount,
        whirlpoolProgram: WHIRLPOOL_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        instructionsSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      })
      .remainingAccounts([
        { pubkey: lvlTickArrayLower, isSigner: false, isWritable: true },
        { pubkey: lvlTickArrayUpper, isSigner: false, isWritable: true },
        { pubkey: lvlWpData.vaultA, isSigner: false, isWritable: true },
        { pubkey: lvlWpData.vaultB, isSigner: false, isWritable: true },
      ])
      .signers([targetBondMint, targetPositionMint])
      .instruction();

    // Build ALT for large transaction
    const slot = await connection.getSlot();
    const [createAltIx, altAddress] = AddressLookupTableProgram.createLookupTable({
      authority: testUser.publicKey,
      payer: testUser.publicKey,
      recentSlot: slot - 1,
    });

    const altAccounts = [
      evolutionConfigPda, levelConfigPda, evoNoncePda,
      layerTokenAuthorityPda, evoBondAuthorityPda,
      levelWhirlpool, levelTokenA, levelLayerToken,
      WHIRLPOOL_PROGRAM_ID, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID,
      lvlWpData.vaultA, lvlWpData.vaultB,
      lvlTickArrayLower, lvlTickArrayUpper,
      currentBondMint, sourceCustodyPda,
      currentBondAccount, userTokenAAccount,
      programTokenAAccount, programTokenBAccount,
      treasuryTokenAccount, targetWhirlpoolPositionPda,
    ];

    const extendAltIx = AddressLookupTableProgram.extendLookupTable({
      payer: testUser.publicKey,
      authority: testUser.publicKey,
      lookupTable: altAddress,
      addresses: altAccounts,
    });

    // Create + extend ALT
    const { blockhash: altBh } = await connection.getLatestBlockhash();
    const altMsg = new TransactionMessage({
      payerKey: testUser.publicKey,
      recentBlockhash: altBh,
      instructions: [createAltIx, extendAltIx],
    }).compileToV0Message();

    const altTx = new VersionedTransaction(altMsg);
    altTx.sign([testUser]);
    const altSig = await connection.sendTransaction(altTx);
    await connection.confirmTransaction(altSig, "confirmed");
    console.log("  ALT created:", altAddress.toBase58());

    // Wait for ALT activation
    console.log("  Waiting for ALT activation (5s)...");
    await new Promise(r => setTimeout(r, 5000));

    const altAccount = await connection.getAddressLookupTable(altAddress);
    if (!altAccount.value) throw new Error("Failed to fetch ALT");

    // Send evolution transaction
    const { blockhash } = await connection.getLatestBlockhash();
    const msgV0 = new TransactionMessage({
      payerKey: testUser.publicKey,
      recentBlockhash: blockhash,
      instructions: [
        ComputeBudgetProgram.setComputeUnitLimit({ units: 800_000 }),
        ComputeBudgetProgram.setComputeUnitPrice({ microLamports: 10_000 }),
        ed25519Ix,
        evolveIx,
      ],
    }).compileToV0Message([altAccount.value]);

    const vTx = new VersionedTransaction(msgV0);
    vTx.sign([testUser, targetBondMint, targetPositionMint]);

    console.log("  Simulating...");
    const simResult = await connection.simulateTransaction(vTx);
    if (simResult.value.err) {
      console.error("  Simulation failed:", JSON.stringify(simResult.value.err));
      if (simResult.value.logs) {
        simResult.value.logs.forEach((l: string) => console.error("    ", l));
      }
      throw new Error(`Evolution to Level ${targetLevel} simulation failed`);
    }

    const evolveSig = await connection.sendTransaction(vTx);
    await connection.confirmTransaction(evolveSig, "confirmed");
    console.log(`  Evolved to Level ${targetLevel}:`, explorerTx(evolveSig));
    console.log("  New bond mint:    ", targetBondMint.publicKey.toBase58());

    // Verify
    const targetBondBalance = await connection.getTokenAccountBalance(userTargetBondAccount);
    console.log("  Target bond balance:", targetBondBalance.value.uiAmount);

    const evoCustody = await evoAccounts.positionCustody.fetch(targetPositionCustodyPda);
    console.log("  Custody level:     ", evoCustody.level);
    console.log("  Custody evolved:   ", evoCustody.isEvolved);
    console.log("  Evolved from:      ", evoCustody.evolvedFrom.toBase58());

    // Update for next iteration
    currentBondMint = targetBondMint.publicKey;
    currentBondAccount = userTargetBondAccount;
    currentCustodyOwner = EVOLUTION_PROGRAM_ID;
  }

  // =========================================================================
  // FINAL VERIFICATION
  // =========================================================================

  console.log("\n" + "=".repeat(70));
  console.log("FINAL VERIFICATION");
  console.log("=".repeat(70));

  const finalBondBalance = await connection.getTokenAccountBalance(currentBondAccount);
  console.log("\nFinal bond (Level 4):", currentBondMint.toBase58());
  console.log("Balance:             ", finalBondBalance.value.uiAmount);

  const finalCustodyPda = PublicKey.findProgramAddressSync(
    [POSITION_CUSTODY_SEED, currentBondMint.toBuffer()],
    EVOLUTION_PROGRAM_ID,
  )[0];
  const finalCustody = await evoAccounts.positionCustody.fetch(finalCustodyPda);
  console.log("Level:               ", finalCustody.level);
  console.log("Is evolved:          ", finalCustody.isEvolved);
  console.log("Depositor:           ", finalCustody.depositor.toBase58());

  const finalEvoConfig = await evoAccounts.evolutionConfig.fetch(evolutionConfigPda);
  console.log("Evolution counter:   ", finalEvoConfig.evolutionCounter.toString());

  console.log("\n" + "=".repeat(70));
  console.log("ALL TESTS PASSED");
  console.log("=".repeat(70));
}

// =============================================================================
// MAIN
// =============================================================================

const command = process.argv[2];

if (command === "generate") {
  generateWallet().catch(console.error);
} else if (command === "run") {
  runTest().catch((err) => {
    console.error("\nTEST FAILED:", err.message || err);
    if (err.logs) {
      console.error("\nProgram Logs:");
      err.logs.forEach((log: string) => console.error("  ", log));
    }
    process.exit(1);
  });
} else {
  console.log("Usage:");
  console.log("  npx ts-node --project tsconfig.scripts.json scripts/user-test.ts generate");
  console.log("  npx ts-node --project tsconfig.scripts.json scripts/user-test.ts run");
}
