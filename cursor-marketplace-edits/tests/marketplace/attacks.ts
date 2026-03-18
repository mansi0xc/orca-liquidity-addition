import * as anchor from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  Keypair,
  PublicKey,
} from "@solana/web3.js";
import {
  getAccount,
  createMint,
  createAccount,
  mintTo,
  approve,
} from "@solana/spl-token";

import {
  setupExchange,
  setupTradeAccounts,
  createOrder,
  computeOrderKeyHash,
  computeOrderHash,
  computeMatchAllowanceHash,
  createEd25519Instruction,
  findOrderFillPDA,
  futureTimestamp,
  pastTimestamp,
  expectError,
  executeMatchOrders,
  sendV0Tx,
  TestContext,
  TradeAccounts,
  MatchOrdersResult,
} from "./helpers";

describe("Exchange - Attack Simulations", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Exchange;
  let ctx: TestContext;
  let trade: TradeAccounts;

  before(async () => {
    ctx = await setupExchange(program, provider);
  });

  beforeEach(async () => {
    trade = await setupTradeAccounts(ctx);
  });

  // ─── B. Negative Tests ────────────────────────────────────────────

  describe("B1: invalid_signature", () => {
    it("should reject when Ed25519 instruction uses wrong signer", async () => {
      const price = 100_000;
      const fakeSigner = Keypair.generate();

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 500,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 501,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();
      const leftKeyHash = computeOrderKeyHash(sellerOrder);
      const rightKeyHash = computeOrderKeyHash(buyerOrder);

      // Build signature instructions but use FAKE signer for order book
      const leftMatchAllowanceHash = computeMatchAllowanceHash(
        leftKeyHash,
        matchLeftTs,
        program.programId
      );
      const ix1 = createEd25519Instruction(fakeSigner, leftMatchAllowanceHash); // WRONG signer

      const leftOrderHash = computeOrderHash(sellerOrder, program.programId);
      const ix2 = createEd25519Instruction(trade.seller, leftOrderHash);

      const rightMatchAllowanceHash = computeMatchAllowanceHash(
        rightKeyHash,
        matchRightTs,
        program.programId
      );
      const ix3 = createEd25519Instruction(fakeSigner, rightMatchAllowanceHash); // WRONG signer

      const rightOrderHash = computeOrderHash(buyerOrder, program.programId);
      const ix4 = createEd25519Instruction(trade.buyer, rightOrderHash);

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
          sigIxOverride: [ix1, ix2, ix3, ix4],
        }),
        "InvalidSignature"
      );
    });
  });

  describe("B2: expired_order", () => {
    it("should reject order with past end timestamp", async () => {
      const price = 100_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 600,
        end: Math.floor(Date.now() / 1000) - 3600, // expired 1 hour ago
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 601,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "OrderExpired"
      );
    });

    it("should reject expired match allowance timestamp", async () => {
      const price = 100_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 610,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 611,
      });

      const matchLeftTs = pastTimestamp(); // EXPIRED
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "MatchAllowanceExpired"
      );
    });
  });

  describe("B3: wrong_price", () => {
    it("should reject when order assets don't match (different mints)", async () => {
      const fakeMint = await createMint(
        provider.connection,
        ctx.owner,
        ctx.owner.publicKey,
        null,
        6
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint, // expects paymentMint
        takeValue: 100_000,
        salt: 700,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: fakeMint, // offers DIFFERENT mint
        makeValue: 100_000,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 701,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "AssetsDoNotMatch"
      );
    });
  });

  describe("B4: seller_not_owner", () => {
    it("should fail when seller doesn't own the NFT (CPI transfer fails)", async () => {
      const price = 100_000;
      const fakeSeller = Keypair.generate();

      const airdropSig = await provider.connection.requestAirdrop(
        fakeSeller.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      // Create NFT account for fakeSeller but DON'T mint to it
      const fakeSellerNftAccount = await createAccount(
        provider.connection,
        ctx.owner,
        trade.nftMint,
        fakeSeller.publicKey
      );

      // Approve exchange_authority on empty account
      await approve(
        provider.connection,
        fakeSeller,
        fakeSellerNftAccount,
        ctx.exchangeAuthority,
        fakeSeller,
        1
      );

      const sellerOrder = createOrder({
        maker: fakeSeller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 800,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 801,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      // Use fakeSellerNftAccount (empty) as NFT source
      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: fakeSellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      // Transfer should fail because fakeSellerNftAccount has 0 NFTs
      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: fakeSeller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: fakeSeller,
        }),
        "Error"
      );
    });
  });

  describe("B5: counterparty_mismatch", () => {
    it("should reject when taker field doesn't match the other maker", async () => {
      const price = 100_000;
      const someoneElse = Keypair.generate();

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        taker: someoneElse.publicKey, // taker is someone else, NOT buyer
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 850,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 851,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "CounterpartyMismatch"
      );
    });
  });

  // ─── C. Attack Simulations ────────────────────────────────────────

  describe("C1: replay_attack_same_signature", () => {
    it("should prevent double execution of the same order (salt > 0)", async () => {
      const price = 100_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 900,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 901,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      // First execution — should succeed
      await executeMatchOrders({
        ctx,
        orderLeft: sellerOrder,
        orderRight: buyerOrder,
        leftMakerKeypair: trade.seller,
        rightMakerKeypair: trade.buyer,
        matchLeftBeforeTimestamp: matchLeftTs,
        matchRightBeforeTimestamp: matchRightTs,
        remainingAccounts,
        payerKeypair: trade.seller,
      });

      // Second execution with same orders — should fail (NothingToFill or CPI error)
      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "Error" // NothingToFill or overflow since fill is already at max
      );
    });
  });

  describe("C2: CPI_account_substitution", () => {
    it("should detect when wrong fee receiver account is passed [SECURITY FINDING]", async () => {
      // This test documents the VULNERABILITY: remaining accounts are NOT validated
      // An attacker could substitute the fee receiver with their own account
      const price = 1_000_000;
      const attacker = Keypair.generate();

      const airdropSig = await provider.connection.requestAirdrop(
        attacker.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      // Create attacker's payment token account
      const attackerPaymentAccount = await createAccount(
        provider.connection,
        ctx.owner,
        trade.paymentMint,
        attacker.publicKey
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 1000,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 1001,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      // ATTACK: substitute fee receiver with attacker's account
      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: attackerPaymentAccount, isWritable: true, isSigner: false }, // ATTACKER SUBSTITUTED
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      // NOTE: This MAY succeed because the program doesn't validate fee receiver
      // If it succeeds, the protocol fees go to the attacker — THIS IS A BUG
      try {
        await executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        });

        // If we get here, the attack succeeded
        const attackerAccount = await getAccount(provider.connection, attackerPaymentAccount);
        if (Number(attackerAccount.amount) > 0) {
          console.log(
            "SECURITY FINDING: Fee receiver substitution attack SUCCEEDED. " +
            `Attacker received ${attackerAccount.amount} tokens as protocol fees.`
          );
        }
      } catch {
        // If it fails, the program has some implicit protection
        console.log("Fee receiver substitution was rejected (implicit protection via CPI).");
      }
    });
  });

  describe("C3: forged_remaining_accounts_payout_diversion", () => {
    it("should document payout diversion via remaining account substitution [SECURITY FINDING]", async () => {
      const price = 500_000;
      const attacker = Keypair.generate();

      const airdropSig = await provider.connection.requestAirdrop(
        attacker.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      const attackerPaymentAccount = await createAccount(
        provider.connection,
        ctx.owner,
        trade.paymentMint,
        attacker.publicKey
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 1100,
        payouts: [{ account: trade.seller.publicKey, value: 10000 }],
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 1101,
        payouts: [{ account: trade.buyer.publicKey, value: 10000 }],
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      // ATTACK: Divert seller's payout to attacker
      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: attackerPaymentAccount, isWritable: true, isSigner: false }, // ATTACKER instead of seller
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      try {
        await executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        });

        const attackerAccount = await getAccount(provider.connection, attackerPaymentAccount);
        if (Number(attackerAccount.amount) > 0) {
          console.log(
            "SECURITY FINDING: Payout diversion attack SUCCEEDED. " +
            `Attacker received ${attackerAccount.amount} tokens that should have gone to seller.`
          );
        }
      } catch {
        console.log("Payout diversion was rejected.");
      }
    });
  });

  describe("C4: maker_not_signer_for_zero_salt", () => {
    it("should reject when non-maker tries to use zero-salt order", async () => {
      const price = 100_000;

      // Seller order with zero salt, but payer is NOT the seller
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 0,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 1200,
      });

      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: new anchor.BN(0),
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: ctx.owner, // NOT the seller — tests non-maker-as-payer
        }),
        "MakerMustBeSignerForZeroSalt"
      );
    });
  });

  describe("C5: collection_bid_blocked", () => {
    it("should reject collection bid orders in match_orders", async () => {
      const price = 100_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 1300,
        collectionBid: true, // COLLECTION BID
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 1301,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "CollectionBidMustUseCollectionBidInstruction"
      );
    });
  });
});
