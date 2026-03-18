import * as anchor from "@coral-xyz/anchor";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  Ed25519Program,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  AddressLookupTableProgram,
  AddressLookupTableAccount,
  VersionedTransaction,
  TransactionMessage,
} from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
  approve,
} from "@solana/spl-token";
import { createHash } from "crypto";

// ─── Constants ───────────────────────────────────────────────────────

export const DATA_TYPE_V1 = Buffer.from([0xa0, 0x83, 0x2e, 0xf7]);
export const DATA_TYPE_EMPTY = Buffer.from([0xff, 0xff, 0xff, 0xff]);

export const EXCHANGE_CONFIG_SEED = "exchange_config";
export const ORDER_FILL_SEED = "order_fill";
export const EXCHANGE_AUTHORITY_SEED = "exchange_authority";
export const ALLOWED_TOKEN_SEED = "allowed_token";
export const FEE_RECEIVER_SEED = "fee_receiver";

// Asset class values (matching Anchor's borsh serialization)
export const AssetClass = {
  Sol: 0,
  WrappedSol: 1,
  SplToken: 2,
  Nft: 3,
  SemiFungible: 4,
} as const;

// ─── Types ───────────────────────────────────────────────────────────

export interface AssetType {
  assetClass: { sol?: {} } | { wrappedSol?: {} } | { splToken?: {} } | { nft?: {} } | { semiFungible?: {} };
  mint: PublicKey;
  tokenId: anchor.BN;
}

export interface Asset {
  assetType: AssetType;
  value: anchor.BN;
}

export interface Part {
  account: PublicKey;
  value: number;
}

export interface Order {
  maker: PublicKey;
  makeAsset: Asset;
  taker: PublicKey;
  takeAsset: Asset;
  salt: anchor.BN;
  start: anchor.BN;
  end: anchor.BN;
  dataType: number[];
  data: Buffer;
  collectionBid: boolean;
}

export interface DataV1 {
  payouts: Part[];
  originFees: Part[];
}

// ─── PDA Derivation ──────────────────────────────────────────────────

export function findExchangeConfigPDA(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(EXCHANGE_CONFIG_SEED)],
    programId
  );
}

export function findOrderFillPDA(orderKeyHash: Buffer, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(ORDER_FILL_SEED), orderKeyHash],
    programId
  );
}

export function findExchangeAuthorityPDA(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(EXCHANGE_AUTHORITY_SEED)],
    programId
  );
}

export function findAllowedTokenPDA(mint: PublicKey, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(ALLOWED_TOKEN_SEED), mint.toBuffer()],
    programId
  );
}

export function findFeeReceiverPDA(mint: PublicKey, programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from(FEE_RECEIVER_SEED), mint.toBuffer()],
    programId
  );
}

// ─── Hash Functions ──────────────────────────────────────────────────

function hashAssetType(assetType: AssetType): Buffer {
  const hash = createHash("sha256");
  let classVal: number;
  if ("sol" in assetType.assetClass) classVal = 0;
  else if ("wrappedSol" in assetType.assetClass) classVal = 1;
  else if ("splToken" in assetType.assetClass) classVal = 2;
  else if ("nft" in assetType.assetClass) classVal = 3;
  else classVal = 4; // semiFungible

  hash.update(Buffer.from([classVal]));
  hash.update(assetType.mint.toBuffer());
  hash.update(assetType.tokenId.toArrayLike(Buffer, "le", 8));
  return hash.digest();
}

export function computeOrderKeyHash(order: Order): Buffer {
  const hash = createHash("sha256");
  hash.update(order.maker.toBuffer());
  hash.update(hashAssetType(order.makeAsset.assetType));
  hash.update(hashAssetType(order.takeAsset.assetType));
  hash.update(order.salt.toArrayLike(Buffer, "le", 8));
  hash.update(Buffer.from([order.collectionBid ? 1 : 0]));
  return hash.digest();
}

export function computeOrderHash(order: Order, programId: PublicKey): Buffer {
  const hash = createHash("sha256");
  hash.update(programId.toBuffer());
  hash.update(Buffer.from("energi"));
  hash.update(Buffer.from([1]));

  hash.update(order.maker.toBuffer());
  hash.update(hashAssetType(order.makeAsset.assetType));
  hash.update(order.makeAsset.value.toArrayLike(Buffer, "le", 8));
  hash.update(order.taker.toBuffer());
  hash.update(hashAssetType(order.takeAsset.assetType));
  hash.update(order.takeAsset.value.toArrayLike(Buffer, "le", 8));
  hash.update(order.salt.toArrayLike(Buffer, "le", 8));
  hash.update(order.start.toArrayLike(Buffer, "le", 8));
  hash.update(order.end.toArrayLike(Buffer, "le", 8));
  hash.update(Buffer.from(order.dataType));

  const dataHash = createHash("sha256").update(order.data).digest();
  hash.update(dataHash);

  hash.update(Buffer.from([order.collectionBid ? 1 : 0]));
  return hash.digest();
}

export function computeMatchAllowanceHash(
  orderKeyHash: Buffer,
  matchBeforeTimestamp: anchor.BN,
  programId: PublicKey
): Buffer {
  const hash = createHash("sha256");
  hash.update(programId.toBuffer());
  hash.update(Buffer.from("energi"));
  hash.update(Buffer.from([1]));
  hash.update(orderKeyHash);
  hash.update(matchBeforeTimestamp.toArrayLike(Buffer, "le", 8));
  return hash.digest();
}

// ─── Order Construction ──────────────────────────────────────────────

export function makeAssetType(
  assetClass: "sol" | "wrappedSol" | "splToken" | "nft" | "semiFungible",
  mint: PublicKey,
  tokenId: number = 0
): AssetType {
  const classMap: Record<string, AssetType["assetClass"]> = {
    sol: { sol: {} },
    wrappedSol: { wrappedSol: {} },
    splToken: { splToken: {} },
    nft: { nft: {} },
    semiFungible: { semiFungible: {} },
  };
  return {
    assetClass: classMap[assetClass],
    mint,
    tokenId: new anchor.BN(tokenId),
  };
}

export function encodeDataV1(data: DataV1): Buffer {
  const parts: Buffer[] = [];

  const payoutsLen = Buffer.alloc(4);
  payoutsLen.writeUInt32LE(data.payouts.length, 0);
  parts.push(payoutsLen);
  for (const p of data.payouts) {
    parts.push(p.account.toBuffer());
    const val = Buffer.alloc(2);
    val.writeUInt16LE(p.value, 0);
    parts.push(val);
  }

  const feesLen = Buffer.alloc(4);
  feesLen.writeUInt32LE(data.originFees.length, 0);
  parts.push(feesLen);
  for (const f of data.originFees) {
    parts.push(f.account.toBuffer());
    const val = Buffer.alloc(2);
    val.writeUInt16LE(f.value, 0);
    parts.push(val);
  }

  return Buffer.concat(parts);
}

export function createOrder(params: {
  maker: PublicKey;
  makeAssetClass: "sol" | "wrappedSol" | "splToken" | "nft" | "semiFungible";
  makeMint: PublicKey;
  makeValue: number;
  taker?: PublicKey;
  takeAssetClass: "sol" | "wrappedSol" | "splToken" | "nft" | "semiFungible";
  takeMint: PublicKey;
  takeValue: number;
  salt?: number;
  start?: number;
  end?: number;
  payouts?: Part[];
  originFees?: Part[];
  collectionBid?: boolean;
}): Order {
  const payouts = params.payouts || [{ account: params.maker, value: 10000 }];
  const originFees = params.originFees || [];
  const dataV1: DataV1 = { payouts, originFees };
  const data = encodeDataV1(dataV1);

  return {
    maker: params.maker,
    makeAsset: {
      assetType: makeAssetType(params.makeAssetClass, params.makeMint),
      value: new anchor.BN(params.makeValue),
    },
    taker: params.taker || PublicKey.default,
    takeAsset: {
      assetType: makeAssetType(params.takeAssetClass, params.takeMint),
      value: new anchor.BN(params.takeValue),
    },
    salt: new anchor.BN(params.salt ?? 1),
    start: new anchor.BN(params.start ?? 0),
    end: new anchor.BN(params.end ?? 0),
    dataType: Array.from(DATA_TYPE_V1),
    data,
    collectionBid: params.collectionBid ?? false,
  };
}

// ─── Ed25519 Signature Helpers ───────────────────────────────────────

export function createEd25519Instruction(
  signer: Keypair,
  message: Buffer
): TransactionInstruction {
  return Ed25519Program.createInstructionWithPrivateKey({
    privateKey: signer.secretKey,
    message: Uint8Array.from(message),
  });
}

export function buildSignatureInstructions(params: {
  orderLeft: Order;
  orderRight: Order;
  orderBookKeypair: Keypair;
  leftMakerKeypair: Keypair;
  rightMakerKeypair: Keypair;
  matchLeftBeforeTimestamp: anchor.BN;
  matchRightBeforeTimestamp: anchor.BN;
  programId: PublicKey;
  payer: PublicKey;
}): TransactionInstruction[] {
  const ixs: TransactionInstruction[] = [];

  if (params.orderLeft.salt.gt(new anchor.BN(0))) {
    const leftKeyHash = computeOrderKeyHash(params.orderLeft);
    const leftMatchAllowanceHash = computeMatchAllowanceHash(
      leftKeyHash,
      params.matchLeftBeforeTimestamp,
      params.programId
    );
    ixs.push(createEd25519Instruction(params.orderBookKeypair, leftMatchAllowanceHash));

    if (!params.payer.equals(params.orderLeft.maker)) {
      const leftOrderHash = computeOrderHash(params.orderLeft, params.programId);
      ixs.push(createEd25519Instruction(params.leftMakerKeypair, leftOrderHash));
    }
  }

  if (params.orderRight.salt.gt(new anchor.BN(0))) {
    const rightKeyHash = computeOrderKeyHash(params.orderRight);
    const rightMatchAllowanceHash = computeMatchAllowanceHash(
      rightKeyHash,
      params.matchRightBeforeTimestamp,
      params.programId
    );
    ixs.push(createEd25519Instruction(params.orderBookKeypair, rightMatchAllowanceHash));

    if (!params.payer.equals(params.orderRight.maker)) {
      const rightOrderHash = computeOrderHash(params.orderRight, params.programId);
      ixs.push(createEd25519Instruction(params.rightMakerKeypair, rightOrderHash));
    }
  }

  return ixs;
}

// ─── Address Lookup Table (ALT) ─────────────────────────────────────

// Shared ALT: created once in setupExchange, extended per-call.
// We track the ALT address and a confirmed-length counter so we can
// poll until the table has grown to the expected size after each extend.
let _altAddress: PublicKey | null = null;
let _altKnownAddresses: Set<string> = new Set();
let _altExpectedLen: number = 0;

async function waitForAlt(
  provider: anchor.AnchorProvider,
  expectedLen: number,
): Promise<AddressLookupTableAccount> {
  for (let i = 0; i < 50; i++) {
    const res = await provider.connection.getAddressLookupTable(_altAddress!);
    if (res.value && res.value.state.addresses.length >= expectedLen) {
      return res.value;
    }
    await new Promise(resolve => setTimeout(resolve, 200));
  }
  throw new Error(`ALT activation timeout (expected ${expectedLen} addresses)`);
}

export async function sendV0Tx(
  ctx: TestContext,
  instructions: TransactionInstruction[],
  payerKeypair: Keypair,
  altAddresses: PublicKey[],
): Promise<string> {
  // Deduplicate and find truly new addresses
  const newAddrs = altAddresses.filter(a => !_altKnownAddresses.has(a.toBase58()));

  if (newAddrs.length > 0) {
    const extIx = AddressLookupTableProgram.extendLookupTable({
      payer: ctx.owner.publicKey,
      authority: ctx.owner.publicKey,
      lookupTable: _altAddress!,
      addresses: newAddrs,
    });
    const extTx = new Transaction().add(extIx);
    await ctx.provider.sendAndConfirm(extTx, [ctx.owner]);
    for (const a of newAddrs) _altKnownAddresses.add(a.toBase58());
    _altExpectedLen += newAddrs.length;
  }

  // Poll until ALT reflects all addresses
  const alt = await waitForAlt(ctx.provider, _altExpectedLen);

  const { blockhash, lastValidBlockHeight } = await ctx.provider.connection.getLatestBlockhash();

  const msg = new TransactionMessage({
    payerKey: payerKeypair.publicKey,
    recentBlockhash: blockhash,
    instructions,
  }).compileToV0Message([alt]);

  const tx = new VersionedTransaction(msg);
  tx.sign([payerKeypair]);

  const txId = await ctx.provider.connection.sendTransaction(tx, {
    skipPreflight: false,
  });
  await ctx.provider.connection.confirmTransaction({
    signature: txId,
    blockhash,
    lastValidBlockHeight,
  });
  return txId;
}

// ─── Match Orders Helper ────────────────────────────────────────────

export interface MatchOrdersParams {
  ctx: TestContext;
  orderLeft: Order;
  orderRight: Order;
  leftMakerKeypair: Keypair;
  rightMakerKeypair: Keypair;
  matchLeftBeforeTimestamp: anchor.BN;
  matchRightBeforeTimestamp: anchor.BN;
  royaltyParts?: Part[];
  remainingAccounts: { pubkey: PublicKey; isWritable: boolean; isSigner: boolean }[];
  payerKeypair?: Keypair;
  sigIxOverride?: TransactionInstruction[];
}

export interface MatchOrdersResult {
  txId: string;
  leftKeyHash: Buffer;
  rightKeyHash: Buffer;
  leftFillPDA: PublicKey;
  rightFillPDA: PublicKey;
}

export async function executeMatchOrders(params: MatchOrdersParams): Promise<MatchOrdersResult> {
  const { ctx } = params;
  const payerKeypair = params.payerKeypair || params.leftMakerKeypair;

  const leftKeyHash = computeOrderKeyHash(params.orderLeft);
  const rightKeyHash = computeOrderKeyHash(params.orderRight);
  const [leftFillPDA] = findOrderFillPDA(leftKeyHash, ctx.program.programId);
  const [rightFillPDA] = findOrderFillPDA(rightKeyHash, ctx.program.programId);

  const sigIxs = params.sigIxOverride ?? buildSignatureInstructions({
    orderLeft: params.orderLeft,
    orderRight: params.orderRight,
    orderBookKeypair: ctx.orderBook,
    leftMakerKeypair: params.leftMakerKeypair,
    rightMakerKeypair: params.rightMakerKeypair,
    matchLeftBeforeTimestamp: params.matchLeftBeforeTimestamp,
    matchRightBeforeTimestamp: params.matchRightBeforeTimestamp,
    programId: ctx.program.programId,
    payer: payerKeypair.publicKey,
  });

  const matchIx = await ctx.program.methods
    .matchOrders({
      leftOrderKeyHash: Array.from(leftKeyHash),
      rightOrderKeyHash: Array.from(rightKeyHash),
      orderLeft: params.orderLeft,
      signatureLeft: Buffer.alloc(0),
      matchLeftBeforeTimestamp: params.matchLeftBeforeTimestamp,
      orderBookSignatureLeft: Buffer.alloc(0),
      orderRight: params.orderRight,
      signatureRight: Buffer.alloc(0),
      matchRightBeforeTimestamp: params.matchRightBeforeTimestamp,
      orderBookSignatureRight: Buffer.alloc(0),
      royaltyParts: params.royaltyParts || [],
    })
    .accounts({
      exchangeConfig: ctx.exchangeConfig,
      payer: payerKeypair.publicKey,
      leftOrderFill: leftFillPDA,
      rightOrderFill: rightFillPDA,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
      exchangeAuthority: ctx.exchangeAuthority,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .remainingAccounts(params.remainingAccounts)
    .instruction();

  const allIxs = [...sigIxs, matchIx];
  const extraAddresses = [
    leftFillPDA,
    rightFillPDA,
    ...params.remainingAccounts.map(a => a.pubkey),
  ];

  const txId = await sendV0Tx(ctx, allIxs, payerKeypair, extraAddresses);
  return { txId, leftKeyHash, rightKeyHash, leftFillPDA, rightFillPDA };
}

// ─── Test Setup Helpers ──────────────────────────────────────────────

export interface TestContext {
  provider: anchor.AnchorProvider;
  program: anchor.Program;
  exchangeConfig: PublicKey;
  exchangeConfigBump: number;
  exchangeAuthority: PublicKey;
  exchangeAuthorityBump: number;
  owner: Keypair;
  exchangeOwner: Keypair;
  orderBook: Keypair;
  feeReceiver: Keypair;
  wsolMint: PublicKey;
  lookupTableAddress: PublicKey;
}

let _sharedContext: TestContext | null = null;

export async function setupExchange(
  program: anchor.Program,
  provider: anchor.AnchorProvider,
): Promise<TestContext> {
  if (_sharedContext) {
    return _sharedContext;
  }

  const owner = (provider.wallet as anchor.Wallet).payer;
  const exchangeOwner = Keypair.generate();
  const orderBook = Keypair.generate();
  const feeReceiver = Keypair.generate();

  const airdropSigs = await Promise.all([
    provider.connection.requestAirdrop(exchangeOwner.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL),
    provider.connection.requestAirdrop(feeReceiver.publicKey, 1 * anchor.web3.LAMPORTS_PER_SOL),
  ]);
  for (const sig of airdropSigs) {
    await provider.connection.confirmTransaction(sig);
  }

  const [exchangeConfig, exchangeConfigBump] = findExchangeConfigPDA(program.programId);
  const [exchangeAuthority, exchangeAuthorityBump] = findExchangeAuthorityPDA(program.programId);

  const wsolMint = await createMint(
    provider.connection,
    owner,
    owner.publicKey,
    null,
    9
  );

  const existingAccount = await provider.connection.getAccountInfo(exchangeConfig);
  if (!existingAccount) {
    await program.methods
      .initialize({
        orderBook: orderBook.publicKey,
        defaultFeeReceiver: feeReceiver.publicKey,
        royaltiesRegistryProgram: PublicKey.default,
        wsolMint: wsolMint,
        exchangeOwner: exchangeOwner.publicKey,
        protocolFeeBps: 250,
      })
      .accounts({
        exchangeConfig,
        authority: owner.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();
  }

  // Create Address Lookup Table with common accounts
  const slot = await provider.connection.getSlot("finalized");
  const [createAltIx, altAddress] = AddressLookupTableProgram.createLookupTable({
    authority: owner.publicKey,
    payer: owner.publicKey,
    recentSlot: slot,
  });

  const commonAddresses = [
    exchangeConfig,
    exchangeAuthority,
    SYSVAR_INSTRUCTIONS_PUBKEY,
    TOKEN_PROGRAM_ID,
    SystemProgram.programId,
    Ed25519Program.programId,
    program.programId,
  ];

  const extendAltIx = AddressLookupTableProgram.extendLookupTable({
    payer: owner.publicKey,
    authority: owner.publicKey,
    lookupTable: altAddress,
    addresses: commonAddresses,
  });

  const altTx = new Transaction().add(createAltIx).add(extendAltIx);
  await provider.sendAndConfirm(altTx, [owner]);

  _altAddress = altAddress;
  for (const a of commonAddresses) _altKnownAddresses.add(a.toBase58());

  // Wait for ALT activation (needs a few slots on localnet)
  await new Promise(resolve => setTimeout(resolve, 1500));

  _sharedContext = {
    provider,
    program,
    exchangeConfig,
    exchangeConfigBump,
    exchangeAuthority,
    exchangeAuthorityBump,
    owner,
    exchangeOwner,
    orderBook,
    feeReceiver,
    wsolMint,
    lookupTableAddress: altAddress,
  };

  return _sharedContext;
}

export interface TradeAccounts {
  seller: Keypair;
  buyer: Keypair;
  nftMint: PublicKey;
  paymentMint: PublicKey;
  sellerNftAccount: PublicKey;
  buyerNftAccount: PublicKey;
  sellerPaymentAccount: PublicKey;
  buyerPaymentAccount: PublicKey;
  feeReceiverPaymentAccount: PublicKey;
}

export async function setupTradeAccounts(
  ctx: TestContext,
  nftAmount: number = 1,
  paymentAmount: number = 1_000_000_000,
): Promise<TradeAccounts> {
  const { provider, exchangeAuthority } = ctx;
  const owner = (provider.wallet as anchor.Wallet).payer;
  const seller = Keypair.generate();
  const buyer = Keypair.generate();

  const airdropSigs = await Promise.all([
    provider.connection.requestAirdrop(seller.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL),
    provider.connection.requestAirdrop(buyer.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL),
  ]);
  for (const sig of airdropSigs) {
    await provider.connection.confirmTransaction(sig);
  }

  const nftMint = await createMint(provider.connection, owner, owner.publicKey, null, 0);
  const paymentMint = await createMint(provider.connection, owner, owner.publicKey, null, 6);

  const sellerNftAccount = await createAccount(provider.connection, owner, nftMint, seller.publicKey);
  const buyerNftAccount = await createAccount(provider.connection, owner, nftMint, buyer.publicKey);
  const sellerPaymentAccount = await createAccount(provider.connection, owner, paymentMint, seller.publicKey);
  const buyerPaymentAccount = await createAccount(provider.connection, owner, paymentMint, buyer.publicKey);
  const feeReceiverPaymentAccount = await createAccount(
    provider.connection, owner, paymentMint, ctx.feeReceiver.publicKey
  );

  await mintTo(provider.connection, owner, nftMint, sellerNftAccount, owner, nftAmount);
  await mintTo(provider.connection, owner, paymentMint, buyerPaymentAccount, owner, paymentAmount);

  await approve(
    provider.connection,
    seller,
    sellerNftAccount,
    exchangeAuthority,
    seller,
    nftAmount
  );

  await approve(
    provider.connection,
    buyer,
    buyerPaymentAccount,
    exchangeAuthority,
    buyer,
    paymentAmount
  );

  return {
    seller,
    buyer,
    nftMint,
    paymentMint,
    sellerNftAccount,
    buyerNftAccount,
    sellerPaymentAccount,
    buyerPaymentAccount,
    feeReceiverPaymentAccount,
  };
}

export function futureTimestamp(secondsAhead: number = 3600): anchor.BN {
  return new anchor.BN(Math.floor(Date.now() / 1000) + secondsAhead);
}

export function pastTimestamp(secondsAgo: number = 3600): anchor.BN {
  return new anchor.BN(Math.floor(Date.now() / 1000) - secondsAgo);
}

export async function expectError(
  fn: () => Promise<any>,
  errorSubstring: string
): Promise<void> {
  try {
    await fn();
    throw new Error(`Expected error containing "${errorSubstring}" but transaction succeeded`);
  } catch (err: any) {
    const errMsg = err.toString();
    const errLogs = err.logs ? err.logs.join("\n") : "";
    const fullMsg = errMsg + "\n" + errLogs;
    if (!fullMsg.includes(errorSubstring)) {
      throw new Error(
        `Expected error containing "${errorSubstring}" but got: ${errMsg}`
      );
    }
  }
}
