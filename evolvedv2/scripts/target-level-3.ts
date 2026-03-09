/**
 * LP Bonds Evolution - Devnet Execution Script
 *
 * This script demonstrates the full evolution flow:
 * 1. Initialize evolution config (admin)
 * 2. Configure levels 2-4 (admin)
 * 3. Evolve a Level 1 bond to Level 2
 *
 * Usage:
 *   npx ts-node scripts/evolve-bond-devnet.ts
 *
 * Prerequisites:
 *   - Devnet SOL in wallet (~/.config/solana/id.json)
 *   - Existing Level 1 bond
 *   - Programs deployed to devnet
 */

import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { LpBondsEvolution } from "../target/types/lp_bonds_evolution";
import {
  PublicKey,
  Keypair,
  ComputeBudgetProgram,
  AddressLookupTableProgram,
  TransactionMessage,
  VersionedTransaction,
  Transaction,
  Ed25519Program,
} from "@solana/web3.js";
import {
  getAssociatedTokenAddress,
  createAssociatedTokenAccountInstruction,
  getAccount,
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import * as nacl from "tweetnacl";
import * as fs from "fs";
import * as path from "path";

// =============================================================================
// CONFIGURATION
// =============================================================================

const CONFIG = {
  // Set to true for admin operations (first-time setup)
  INITIALIZE_MODE: false,

  // The source Level 1 bond to evolve
  SOURCE_BOND_MINT: new PublicKey("CL9ZgeuW3NVostFQoSJRGT4zDFtysLLnPj34doYvzDZz"), // Set this before running

  // Target level for evolution (2, 3, or 4)
  TARGET_LEVEL: 3,

  // Compute budget (increase if needed)
  COMPUTE_UNITS: 400_000,
  PRIORITY_FEE: 100_000, // lamports

  // Oracle keypair path (for signing evolution requests)
  ORACLE_KEYPAIR_PATH: path.join(
    process.env.HOME || "~",
    ".config/solana/id.json"
  ),
};

// =============================================================================
// CONSTANTS
// =============================================================================

const WHIRLPOOL_PROGRAM_ID = new PublicKey(
  "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc"
);

const LP_BONDS_PROGRAM_ID = new PublicKey(
  "AmJcNFdgckd1o6DPa6j12WGM6wNKZdvdWphtsP2Ws92w"
);

// Whirlpools by level
const WHIRLPOOLS = {
  1: new PublicKey("8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN"),
  2: new PublicKey("36whP2YDjunT6VNCCPEn1MV9BrZxc5XsD7tAJMVahr1V"),
  3: new PublicKey("GMNFmkhU8hnCwofqh9gGwW8H6SqohrP8PmoJQAMycNwZ"),
  4: new PublicKey("2bdPMRcKrgAvQKGfP1mW9ThNjq6rnP2nRSYmWodtdFvo"),
};

// Token mints by level (Token B / Layer tokens)
const LAYER_TOKENS = {
  1: new PublicKey("4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"),
  2: new PublicKey("Ci3iuaCJfQAapWHJkfycuTc67SCEZYfKTS8fxjKCP5tB"),
  3: new PublicKey("9b7gAMUxGdRwkEk32KtayLXAhwqib3yaTzLdvtMfvXbp"),
  4: new PublicKey("9Zs8kUpicKNZNosFwMawxnVqFZxBfZz8dh2zLu2wahnu"),
};

// PDA Seeds
const EVOLUTION_CONFIG_SEED = Buffer.from("evolution_config");
const LEVEL_CONFIG_SEED = Buffer.from("level_config");
const EVOLUTION_RECORD_SEED = Buffer.from("evolution_record");
const LAYER_TOKEN_AUTHORITY_SEED = Buffer.from("layer_token_authority");
const BOND_AUTHORITY_SEED = Buffer.from("bond_authority");
const POSITION_CUSTODY_SEED = Buffer.from("position_custody");
const NONCE_SEED = Buffer.from("nonce");

const EVOLUTION_SIGNATURE_DOMAIN = Buffer.from("LP_BONDS_EVOLVE_V1");

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function loadKeypair(filePath: string): Keypair {
  const secretKey = JSON.parse(fs.readFileSync(filePath, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}

async function derivePda(
  seeds: (Buffer | Uint8Array)[],
  programId: PublicKey
): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddressSync(seeds, programId);
}

function createEvolutionCanonicalMessage(params: {
  sourceBondMint: PublicKey;
  targetLevel: number;
  amountA: BN;
  amountB: BN;
  liquidity: BN;
  nonce: BN;
  sender: PublicKey;
  contractAddress: PublicKey;
}): Buffer {
  const buffer = Buffer.alloc(155);
  let offset = 0;

  EVOLUTION_SIGNATURE_DOMAIN.copy(buffer, offset);
  offset += 18;

  params.sourceBondMint.toBuffer().copy(buffer, offset);
  offset += 32;

  buffer.writeUInt8(params.targetLevel, offset);
  offset += 1;

  buffer.writeBigUInt64LE(BigInt(params.amountA.toString()), offset);
  offset += 8;

  buffer.writeBigUInt64LE(BigInt(params.amountB.toString()), offset);
  offset += 8;

  const liquidityBuf = Buffer.alloc(16);
  const liq = BigInt(params.liquidity.toString());
  liquidityBuf.writeBigUInt64LE(liq & BigInt("0xFFFFFFFFFFFFFFFF"), 0);
  liquidityBuf.writeBigUInt64LE(liq >> BigInt(64), 8);
  liquidityBuf.copy(buffer, offset);
  offset += 16;

  buffer.writeBigUInt64LE(BigInt(params.nonce.toString()), offset);
  offset += 8;

  params.sender.toBuffer().copy(buffer, offset);
  offset += 32;

  params.contractAddress.toBuffer().copy(buffer, offset);

  return buffer;
}

function getExplorerUrl(
  signature: string,
  cluster: "devnet" | "mainnet-beta" = "devnet"
): string {
  return `https://explorer.solana.com/tx/${signature}?cluster=${cluster}`;
}

// =============================================================================
// MAIN SCRIPT
// =============================================================================

async function main() {
  console.log("╔════════════════════════════════════════════════════════════╗");
  console.log("║           LP Bonds Evolution - Devnet Script               ║");
  console.log("╚════════════════════════════════════════════════════════════╝");
  console.log();

  // Setup provider
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const evolutionProgram = anchor.workspace
    .LpBondsEvolution as Program<LpBondsEvolution>;

  const wallet = provider.wallet as anchor.Wallet;
  console.log("Wallet:", wallet.publicKey.toString());
  console.log("Evolution Program:", evolutionProgram.programId.toString());
  console.log();

  // Derive PDAs
  const [evolutionConfigPda] = await derivePda(
    [EVOLUTION_CONFIG_SEED],
    evolutionProgram.programId
  );
  const [layerTokenAuthorityPda] = await derivePda(
    [LAYER_TOKEN_AUTHORITY_SEED],
    evolutionProgram.programId
  );
  const [bondAuthorityPda] = await derivePda(
    [BOND_AUTHORITY_SEED],
    evolutionProgram.programId
  );

  console.log("PDAs:");
  console.log("  Evolution Config:", evolutionConfigPda.toString());
  console.log("  Layer Token Authority:", layerTokenAuthorityPda.toString());
  console.log("  Bond Authority:", bondAuthorityPda.toString());
  console.log();

  // Check if evolution config exists
  let configExists = false;
  try {
    await evolutionProgram.account.evolutionConfig.fetch(evolutionConfigPda);
    configExists = true;
    console.log("✓ Evolution config already initialized");
  } catch {
    console.log("○ Evolution config not initialized");
  }

  // ==========================================================================
  // ADMIN INITIALIZATION MODE
  // ==========================================================================

  if (CONFIG.INITIALIZE_MODE) {
    console.log("\n─── ADMIN INITIALIZATION ───");

    // Load or generate oracle keypair
    let oracleKeypair: Keypair;
    if (fs.existsSync(CONFIG.ORACLE_KEYPAIR_PATH)) {
      oracleKeypair = loadKeypair(CONFIG.ORACLE_KEYPAIR_PATH);
      console.log("Loaded oracle keypair:", oracleKeypair.publicKey.toString());
    } else {
      oracleKeypair = Keypair.generate();
      fs.writeFileSync(
        CONFIG.ORACLE_KEYPAIR_PATH,
        JSON.stringify(Array.from(oracleKeypair.secretKey))
      );
      console.log(
        "Generated new oracle keypair:",
        oracleKeypair.publicKey.toString()
      );
    }

    // Treasury (can be updated later)
    const treasury = wallet.publicKey;

    if (!configExists) {
      // Step 1: Initialize evolution config
      console.log("\n1. Initializing evolution config...");
      const initTx = await evolutionProgram.methods
        .initializeEvolution(treasury, oracleKeypair.publicKey)
        .accounts({
          admin: wallet.publicKey,
        })
        .preInstructions([
          ComputeBudgetProgram.setComputeUnitLimit({
            units: CONFIG.COMPUTE_UNITS,
          }),
          ComputeBudgetProgram.setComputeUnitPrice({
            microLamports: CONFIG.PRIORITY_FEE,
          }),
        ])
        .rpc();

      console.log("   ✓ Tx:", getExplorerUrl(initTx));

      // Step 2: Initialize layer token authority
      console.log("\n2. Initializing layer token authority...");
      const authTx = await evolutionProgram.methods
        .initializeLayerAuthority()
        .accounts({
          admin: wallet.publicKey,
        })
        .rpc();

      console.log("   ✓ Tx:", getExplorerUrl(authTx));
    } else {
      console.log("\n1. Evolution config already exists, skipping init...");
      console.log("2. Layer token authority already exists, skipping init...");
    }

    // Step 3: Create layer token mints for levels 2-4 and configure levels
    const createdLayerMints: { [level: number]: PublicKey } = {};
    
    for (let level = 2; level <= 4; level++) {
      const [levelConfigPda] = await derivePda(
        [LEVEL_CONFIG_SEED, Buffer.from([level])],
        evolutionProgram.programId
      );

      // Check if level config already exists
      let existingLayerMint: PublicKey | null = null;
      try {
        const existingConfig = await evolutionProgram.account.levelConfig.fetch(levelConfigPda);
        existingLayerMint = existingConfig.layerTokenMint;
        console.log(`\n3.${level - 1}a. Level ${level} config exists, reusing layer mint: ${existingLayerMint.toString()}`);
      } catch {
        // Level config doesn't exist, create layer mint
        console.log(`\n3.${level - 1}a. Creating Layer Token Mint for Level ${level}...`);

        const layerMintKeypair = Keypair.generate();
        createdLayerMints[level] = layerMintKeypair.publicKey;
        existingLayerMint = layerMintKeypair.publicKey;

        const createMintTx = await evolutionProgram.methods
          .createLayerTokenMint(9) // 9 decimals like most Solana tokens
          .accounts({
            admin: wallet.publicKey,
            layerTokenMint: layerMintKeypair.publicKey,
          })
          .signers([layerMintKeypair])
          .rpc();

        console.log(`   ✓ Layer ${level} Mint:`, layerMintKeypair.publicKey.toString());
        console.log(`   ✓ Tx:`, getExplorerUrl(createMintTx));
      }

      console.log(`\n3.${level - 1}b. Configuring Level ${level}...`);

      const whirlpool = WHIRLPOOLS[level as keyof typeof WHIRLPOOLS];

      // Lock durations: 60, 90, 120 days
      const lockDuration = [0, 0, 5_184_000, 7_776_000, 10_368_000][level];

      // Fee: 1%, 1.5%, 2%
      const feeBps = [0, 0, 100, 150, 200][level];

      // Multiplier: 1.5x, 2x, 3x
      const multiplier = [0, 0, 150, 200, 300][level];

      const configTx = await evolutionProgram.methods
        .configureLevel(
          level,
          -443520, // tick_lower (full range, aligned to tick spacing 128)
          443520, // tick_upper (aligned to tick spacing 128)
          new BN(1_000_000_000 * level), // required_amount_a
          new BN(500_000_000 * level), // required_amount_b
          feeBps,
          new BN(lockDuration),
          multiplier,
          true // is_active
        )
        .accounts({
          admin: wallet.publicKey,
          levelConfig: levelConfigPda,
          whirlpool: whirlpool,
          tokenMintA: LAYER_TOKENS[1], // Base token (Level 1 layer token)
          tokenMintB: existingLayerMint!,
          layerTokenMint: existingLayerMint!,
        })
        .rpc();

      console.log(`   ✓ Level ${level} Config Tx:`, getExplorerUrl(configTx));
    }
    
    console.log("\n--- Layer Token Mints ---");
    console.log("(Fetch from level configs for actual values)")

    console.log("\n✓ Admin initialization complete!");
    return;
  }

  // ==========================================================================
  // EVOLUTION MODE
  // ==========================================================================

  if (!CONFIG.SOURCE_BOND_MINT) {
    console.log("\n⚠ SOURCE_BOND_MINT not set in config. Set it to evolve a bond.");
    console.log("  Example: CONFIG.SOURCE_BOND_MINT = new PublicKey('...');");
    return;
  }

  console.log("\n─── BOND EVOLUTION ───");
  console.log("Source Bond Mint:", CONFIG.SOURCE_BOND_MINT.toString());
  console.log("Target Level:", CONFIG.TARGET_LEVEL);

  // Load oracle keypair
  if (!fs.existsSync(CONFIG.ORACLE_KEYPAIR_PATH)) {
    console.error("ERROR: Oracle keypair not found at", CONFIG.ORACLE_KEYPAIR_PATH);
    return;
  }
  const oracleKeypair = loadKeypair(CONFIG.ORACLE_KEYPAIR_PATH);

  // Fetch source custody - for evolved bonds (L2+), use evolution program ID
  // For L1 bonds, use LP_BONDS_PROGRAM_ID
  // Since we're evolving L2→L3, the source custody was created by evolution program
  const [sourceCustodyPda] = await derivePda(
    [POSITION_CUSTODY_SEED, CONFIG.SOURCE_BOND_MINT.toBuffer()],
    evolutionProgram.programId // Use evolution program for L2+ bonds
  );

  console.log("Source Custody PDA:", sourceCustodyPda.toString());

  // Fetch evolution config
  const evolutionConfig = await evolutionProgram.account.evolutionConfig.fetch(
    evolutionConfigPda
  );

  if (evolutionConfig.isPaused) {
    console.error("ERROR: Evolution is currently paused");
    return;
  }

  // Fetch level config
  const [levelConfigPda] = await derivePda(
    [LEVEL_CONFIG_SEED, Buffer.from([CONFIG.TARGET_LEVEL])],
    evolutionProgram.programId
  );

  const levelConfig = await evolutionProgram.account.levelConfig.fetch(
    levelConfigPda
  );

  if (!levelConfig.isActive) {
    console.error("ERROR: Target level is not active");
    return;
  }

  console.log("\nLevel Config:");
  console.log("  Whirlpool:", levelConfig.whirlpool.toString());
  console.log("  Required Amount A:", levelConfig.requiredAmountA.toString());
  console.log("  Required Amount B:", levelConfig.requiredAmountB.toString());
  console.log("  Fee BPS:", levelConfig.feeBps);
  console.log("  Lock Duration:", levelConfig.lockDuration.toString(), "seconds");

  // Fetch nonce
  const [noncePda] = await derivePda(
    [NONCE_SEED, wallet.publicKey.toBuffer()],
    LP_BONDS_PROGRAM_ID
  );

  let currentNonce = new BN(0);
  try {
    const nonceAccount = await provider.connection.getAccountInfo(noncePda);
    if (nonceAccount) {
      currentNonce = new BN(nonceAccount.data.subarray(40, 48), "le");
    }
  } catch {
    console.log("No nonce account found, using 0");
  }

  const newNonce = currentNonce.add(new BN(1));
  console.log("\nNonce:", currentNonce.toString(), "->", newNonce.toString());

  // Define liquidity amount (must match what's used in evolveBond call)
  // For full-range positions, liquidity needs to be small enough that 
  // token amounts needed don't exceed what user deposits (amount_a, amount_b)
  const liquidityAmount = new BN("1000000"); // Small liquidity for testing

  // Create signature
  const evolutionMessage = createEvolutionCanonicalMessage({
    sourceBondMint: CONFIG.SOURCE_BOND_MINT,
    targetLevel: CONFIG.TARGET_LEVEL,
    amountA: levelConfig.requiredAmountA,
    amountB: levelConfig.requiredAmountB,
    liquidity: liquidityAmount,
    nonce: newNonce,
    sender: wallet.publicKey,
    contractAddress: evolutionProgram.programId,
  });

  const signature = nacl.sign.detached(evolutionMessage, oracleKeypair.secretKey);
  console.log("Signature generated:", Buffer.from(signature).toString("hex").slice(0, 32) + "...");

  // Derive all required accounts
  const targetBondMint = Keypair.generate();
  const positionMint = Keypair.generate();

  const [targetCustodyPda] = await derivePda(
    [POSITION_CUSTODY_SEED, targetBondMint.publicKey.toBuffer()],
    evolutionProgram.programId
  );

  const [evolutionRecordPda] = await derivePda(
    [EVOLUTION_RECORD_SEED, CONFIG.SOURCE_BOND_MINT.toBuffer()],
    evolutionProgram.programId
  );

  const [whirlpoolPositionPda] = PublicKey.findProgramAddressSync(
    [Buffer.from("position"), positionMint.publicKey.toBuffer()],
    WHIRLPOOL_PROGRAM_ID
  );

  console.log("\nTarget Accounts:");
  console.log("  Target Bond Mint:", targetBondMint.publicKey.toString());
  console.log("  Position Mint:", positionMint.publicKey.toString());
  console.log("  Target Custody:", targetCustodyPda.toString());
  console.log("  Evolution Record:", evolutionRecordPda.toString());
  console.log("  Whirlpool Position:", whirlpoolPositionPda.toString());

  console.log("\n⚠ Evolution transaction ready to execute.");
  console.log("  Review the accounts above and ensure you have sufficient tokens.");
  console.log("  The execution requires:");
  console.log("  - User must hold the source bond NFT");
  console.log("  - User must have sufficient Token A balance");
  console.log("  - Tick arrays must be initialized for the target whirlpool");
  console.log("  - Treasury token account must exist");

  // Fetch whirlpool account data for vaults
  const whirlpoolAccountInfo = await provider.connection.getAccountInfo(levelConfig.whirlpool);
  if (!whirlpoolAccountInfo) {
    throw new Error("Whirlpool account not found");
  }

  // Whirlpool layout (after 8-byte discriminator):
  // whirlpools_config: 32, bump: 1, tick_spacing: 2, tick_spacing_seed: 2,
  // fee_rate: 2, protocol_fee_rate: 2, liquidity: 16, sqrt_price: 16,
  // tick_current_index: 4, protocol_fee_owed_a: 8, protocol_fee_owed_b: 8,
  // token_mint_a: 32, token_vault_a: 32, fee_growth_global_a: 16,
  // token_mint_b: 32, token_vault_b: 32, ...
  const whirlpoolData = whirlpoolAccountInfo.data;
  const tokenVaultAOffset = 8 + 32 + 1 + 2 + 2 + 2 + 2 + 16 + 16 + 4 + 8 + 8 + 32; // = 133
  const tokenVaultBOffset = tokenVaultAOffset + 32 + 16 + 32; // = 213
  const tokenVaultA = new PublicKey(whirlpoolData.subarray(tokenVaultAOffset, tokenVaultAOffset + 32));
  const tokenVaultB = new PublicKey(whirlpoolData.subarray(tokenVaultBOffset, tokenVaultBOffset + 32));

  console.log("  Token Vault A:", tokenVaultA.toString());
  console.log("  Token Vault B:", tokenVaultB.toString());

  // Derive tick arrays for the whirlpool based on tick spacing
  // Fetch tick spacing from whirlpool account
  const tickSpacing = whirlpoolData.readUInt16LE(8 + 32 + 1); // After discriminator(8) + config(32) + bump(1)
  console.log("  Tick Spacing:", tickSpacing);
  
  const ticksPerArray = 88 * tickSpacing;
  const tickArrayLowerStartIndex = Math.floor(levelConfig.tickLower / ticksPerArray) * ticksPerArray;
  const tickArrayUpperStartIndex = Math.floor(levelConfig.tickUpper / ticksPerArray) * ticksPerArray;

  const [tickArrayLower] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("tick_array"),
      levelConfig.whirlpool.toBuffer(),
      Buffer.from(tickArrayLowerStartIndex.toString()),
    ],
    WHIRLPOOL_PROGRAM_ID
  );

  const [tickArrayUpper] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("tick_array"),
      levelConfig.whirlpool.toBuffer(),
      Buffer.from(tickArrayUpperStartIndex.toString()),
    ],
    WHIRLPOOL_PROGRAM_ID
  );

  console.log("  Tick Array Lower:", tickArrayLower.toString());
  console.log("  Tick Array Upper:", tickArrayUpper.toString());

  // NOTE: Evolution execution is commented out by default.
  // Uncomment the block below to execute after ensuring all prerequisites are met.
  console.log("\n⚠ Evolution execution is commented out. Review prerequisites and uncomment to execute.");

  // Build accounts - only pass non-auto-derived accounts
  const userSourceBondAccount = await getAssociatedTokenAddress(CONFIG.SOURCE_BOND_MINT, wallet.publicKey);
  const userTokenAAccount = await getAssociatedTokenAddress(levelConfig.tokenMintA, wallet.publicKey);
  const userTokenBAccount = await getAssociatedTokenAddress(levelConfig.layerTokenMint, wallet.publicKey);
  const treasuryTokenAccount = await getAssociatedTokenAddress(levelConfig.tokenMintA, evolutionConfig.treasury);
  const positionTokenAccount = await getAssociatedTokenAddress(positionMint.publicKey, wallet.publicKey);
  const custodyPositionTokenAccount = await getAssociatedTokenAddress(positionMint.publicKey, layerTokenAuthorityPda, true);

  // Check and create user Token A account if it doesn't exist
  console.log("\nChecking prerequisite accounts...");
  console.log("  Token A Mint:", levelConfig.tokenMintA.toString());
  console.log("  User Token A Account:", userTokenAAccount.toString());
  
  try {
    await getAccount(provider.connection, userTokenAAccount);
    console.log("  ✓ User Token A account exists");
  } catch {
    console.log("  Creating User Token A account...");
    const createIx = createAssociatedTokenAccountInstruction(
      wallet.publicKey,
      userTokenAAccount,
      wallet.publicKey,
      levelConfig.tokenMintA
    );
    const tx = new Transaction().add(createIx);
    const sig = await provider.sendAndConfirm(tx);
    console.log("  ✓ Created:", getExplorerUrl(sig));
  }

  // Check and create treasury token account if it doesn't exist
  console.log("  Treasury Token Account:", treasuryTokenAccount.toString());
  try {
    await getAccount(provider.connection, treasuryTokenAccount);
    console.log("  ✓ Treasury Token account exists");
  } catch {
    console.log("  Creating Treasury Token account...");
    const createIx = createAssociatedTokenAccountInstruction(
      wallet.publicKey,
      treasuryTokenAccount,
      evolutionConfig.treasury,
      levelConfig.tokenMintA
    );
    const tx = new Transaction().add(createIx);
    const sig = await provider.sendAndConfirm(tx);
    console.log("  ✓ Created:", getExplorerUrl(sig));
  }

  console.log("\n🚀 Creating Address Lookup Table for large transaction...");

  // Create Address Lookup Table to reduce transaction size
  const slot = await provider.connection.getSlot();
  const [createAltIx, altAddress] = AddressLookupTableProgram.createLookupTable({
    authority: wallet.publicKey,
    payer: wallet.publicKey,
    recentSlot: slot - 1,
  });

  // Add addresses to the lookup table - include as many as possible
  const accountsToAdd = [
    evolutionConfigPda,
    levelConfigPda,
    layerTokenAuthorityPda,
    bondAuthorityPda,
    levelConfig.whirlpool,
    levelConfig.tokenMintA,
    levelConfig.layerTokenMint,
    WHIRLPOOL_PROGRAM_ID,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID,
    tokenVaultA,
    tokenVaultB,
    tickArrayLower,
    tickArrayUpper,
    noncePda,
    CONFIG.SOURCE_BOND_MINT,
    sourceCustodyPda,
    userSourceBondAccount,
    userTokenAAccount,
    userTokenBAccount,
    treasuryTokenAccount,
    whirlpoolPositionPda,
    evolutionProgram.programId,
    oracleKeypair.publicKey,
  ];

  const extendAltIx = AddressLookupTableProgram.extendLookupTable({
    payer: wallet.publicKey,
    authority: wallet.publicKey,
    lookupTable: altAddress,
    addresses: accountsToAdd,
  });

  // Create and extend ALT
  const { blockhash: altBlockhash } = await provider.connection.getLatestBlockhash();
  const altMessage = new TransactionMessage({
    payerKey: wallet.publicKey,
    recentBlockhash: altBlockhash,
    instructions: [createAltIx, extendAltIx],
  }).compileToV0Message();

  const altTx = new VersionedTransaction(altMessage);
  altTx.sign([wallet.payer]);
  
  const altSig = await provider.connection.sendTransaction(altTx);
  await provider.connection.confirmTransaction(altSig);
  console.log("  ALT created:", altAddress.toString());
  console.log("  Tx:", getExplorerUrl(altSig));

  // Wait for ALT to be usable (needs a few slots - ~500ms per slot)
  console.log("  Waiting for ALT activation (5 seconds)...");
  await new Promise(resolve => setTimeout(resolve, 5000));

  // Fetch the ALT
  const altAccount = await provider.connection.getAddressLookupTable(altAddress);
  if (!altAccount.value) {
    throw new Error("Failed to fetch ALT");
  }

  console.log("\n🚀 Executing evolution transaction with ALT...");

  // Build the evolution instruction
  const evolveIx = await evolutionProgram.methods
    .evolveBond(
      CONFIG.TARGET_LEVEL,
      levelConfig.requiredAmountA,
      levelConfig.requiredAmountB,
      liquidityAmount, // must match what's in the signature
      levelConfig.requiredAmountA, // token_max_a = amount deposited
      levelConfig.requiredAmountB, // token_max_b = amount deposited
      levelConfig.tickLower,
      levelConfig.tickUpper,
      newNonce
    )
    .accounts({
      user: wallet.publicKey,
      levelConfig: levelConfigPda,
      nonceAccount: noncePda,
      sourceBondMint: CONFIG.SOURCE_BOND_MINT,
      userSourceBondAccount: userSourceBondAccount,
      sourceCustody: sourceCustodyPda,
      targetBondMint: targetBondMint.publicKey,
      tokenMintA: levelConfig.tokenMintA,
      layerTokenMint: levelConfig.layerTokenMint,
      userTokenAAccount: userTokenAAccount,
      userTokenBAccount: userTokenBAccount,
      treasuryTokenAccount: treasuryTokenAccount,
      whirlpool: levelConfig.whirlpool,
      tokenVaultA: tokenVaultA,
      tokenVaultB: tokenVaultB,
      positionMint: positionMint.publicKey,
      whirlpoolPosition: whirlpoolPositionPda,
      positionTokenAccount: positionTokenAccount,
      custodyPositionTokenAccount: custodyPositionTokenAccount,
    })
    .remainingAccounts([
      { pubkey: tickArrayLower, isSigner: false, isWritable: true },
      { pubkey: tickArrayUpper, isSigner: false, isWritable: true },
    ])
    .signers([targetBondMint, positionMint])
    .instruction();

  // Create Ed25519 program instruction for signature verification
  const ed25519Ix = Ed25519Program.createInstructionWithPublicKey({
    publicKey: oracleKeypair.publicKey.toBytes(),
    message: evolutionMessage,
    signature: signature,
  });

  // Create versioned transaction with ALT
  // Note: Ed25519 instruction must precede evolveBond for signature verification
  const { blockhash } = await provider.connection.getLatestBlockhash();
  const messageV0 = new TransactionMessage({
    payerKey: wallet.publicKey,
    recentBlockhash: blockhash,
    instructions: [
      ComputeBudgetProgram.setComputeUnitLimit({ units: CONFIG.COMPUTE_UNITS }),
      ComputeBudgetProgram.setComputeUnitPrice({ microLamports: CONFIG.PRIORITY_FEE }),
      ed25519Ix,
      evolveIx,
    ],
  }).compileToV0Message([altAccount.value]);
  
  console.log("  Instruction data length:", evolveIx.data.length);
  console.log("  Number of accounts:", evolveIx.keys.length);
  console.log("  Ed25519 instruction data length:", ed25519Ix.data.length);

  const versionedTx = new VersionedTransaction(messageV0);
  versionedTx.sign([wallet.payer, targetBondMint, positionMint]);

  // First simulate to get full logs
  console.log("\n  Simulating transaction...");
  try {
    const simResult = await provider.connection.simulateTransaction(versionedTx, {
      sigVerify: false,
      replaceRecentBlockhash: true,
    });
    
    if (simResult.value.err) {
      console.log("\n  === SIMULATION FAILED ===");
      console.log("  Error:", JSON.stringify(simResult.value.err, null, 2));
      console.log("\n  Full logs:");
      simResult.value.logs?.forEach((log, i) => {
        console.log(`    ${i}: ${log}`);
      });
      throw new Error("Simulation failed");
    }
    console.log("  Simulation passed!");
  } catch (simErr) {
    console.error("\n  Simulation error:", simErr);
    throw simErr;
  }

  const evolveTx = await provider.connection.sendTransaction(versionedTx);
  await provider.connection.confirmTransaction(evolveTx);

  console.log("\n✓ Evolution complete!");
  console.log("  Tx:", getExplorerUrl(evolveTx));
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err);
    process.exit(1);
  });
