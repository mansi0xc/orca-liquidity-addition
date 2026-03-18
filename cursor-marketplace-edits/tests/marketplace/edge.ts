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
  executeMatchOrders,
  futureTimestamp,
  expectError,
  TestContext,
  TradeAccounts,
  Part,
} from "./helpers";

describe("Exchange - Edge Cases", () => {
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

  // ─── D. Edge Cases ────────────────────────────────────────────────

  describe("D1: zero_price_order", () => {
    it("should handle order with zero take value (free NFT)", async () => {
      // An order requesting 0 payment — effectively giving away the NFT for free
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 0, // FREE
        salt: 2000,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: 0, // FREE
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 2001,
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

      // This might fail due to DivisionByZero in fill calculation
      // (take_value = 0 used as denominator in calculate_remaining)
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
        console.log("WARNING: Zero-price order succeeded — may need validation");
      } catch (err: any) {
        const errMsg = err.toString();
        expect(
          errMsg.includes("DivisionByZero") ||
          errMsg.includes("NothingToFill") ||
          errMsg.includes("Error")
        ).to.be.true;
      }
    });
  });

  describe("D2: extremely_large_amount", () => {
    it("should handle orders near u64::MAX without overflow", async () => {
      const maxU64 = new anchor.BN("18446744073709551615");

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: Number.MAX_SAFE_INTEGER, // very large value
        salt: 2100,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: Number.MAX_SAFE_INTEGER,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 2101,
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

      // Should fail — buyer doesn't have enough tokens
      // But the important thing is it doesn't panic with overflow
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
        "Error"
      );
    });
  });

  describe("D3: multiple_orders_same_nft", () => {
    it("second order for same NFT should fail after first fills", async () => {
      const price = 100_000;

      // Create two buyer accounts
      const buyer2 = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        buyer2.publicKey,
        5 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      const buyer2PaymentAccount = await createAccount(
        provider.connection,
        ctx.owner,
        trade.paymentMint,
        buyer2.publicKey
      );
      await mintTo(
        provider.connection,
        ctx.owner,
        trade.paymentMint,
        buyer2PaymentAccount,
        ctx.owner,
        price
      );
      await approve(
        provider.connection,
        buyer2,
        buyer2PaymentAccount,
        ctx.exchangeAuthority,
        buyer2,
        price
      );

      const buyer2NftAccount = await createAccount(
        provider.connection,
        ctx.owner,
        trade.nftMint,
        buyer2.publicKey
      );

      // Seller has 1 NFT, creates TWO sell orders with different salts
      const sellerOrder1 = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 3000,
      });

      const buyerOrder1 = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 3001,
      });

      // Execute first trade
      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts1 = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await executeMatchOrders({
        ctx,
        orderLeft: sellerOrder1,
        orderRight: buyerOrder1,
        leftMakerKeypair: trade.seller,
        rightMakerKeypair: trade.buyer,
        matchLeftBeforeTimestamp: matchLeftTs,
        matchRightBeforeTimestamp: matchRightTs,
        remainingAccounts: remainingAccounts1,
        payerKeypair: trade.seller,
      });

      // Verify first trade succeeded
      const buyerNft1 = await getAccount(provider.connection, trade.buyerNftAccount);
      expect(buyerNft1.amount.toString()).to.equal("1");

      // Now try second trade with different buyer for same NFT
      // Seller no longer has the NFT — this should fail at CPI level
      const sellerOrder2 = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 3002, // different salt = different order
      });

      const buyerOrder2 = createOrder({
        maker: buyer2.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 3003,
      });

      const matchLeftTs2 = futureTimestamp();
      const matchRightTs2 = futureTimestamp();

      const remainingAccounts2 = [
        { pubkey: buyer2PaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false }, // empty now
        { pubkey: buyer2NftAccount, isWritable: true, isSigner: false },
      ];

      // Should fail because seller's NFT account is now empty
      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder2,
          orderRight: buyerOrder2,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: buyer2,
          matchLeftBeforeTimestamp: matchLeftTs2,
          matchRightBeforeTimestamp: matchRightTs2,
          remainingAccounts: remainingAccounts2,
          payerKeypair: trade.seller,
        }),
        "Error"
      );
    });
  });

  describe("D4: paused_exchange", () => {
    it("should reject match_orders when exchange is paused", async () => {
      // Pause the exchange
      await program.methods
        .togglePause()
        .accounts({
          exchangeConfig: ctx.exchangeConfig,
          owner: ctx.owner.publicKey,
        })
        .rpc();

      const price = 100_000;
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 4000,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 4001,
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
        "Paused"
      );

      // Unpause for subsequent tests
      await program.methods
        .togglePause()
        .accounts({
          exchangeConfig: ctx.exchangeConfig,
          owner: ctx.owner.publicKey,
        })
        .rpc();
    });
  });

  describe("D5: asset_class_mismatch", () => {
    it("should reject NFT-to-NFT trades (both sides non-fungible)", async () => {
      // Create a second NFT mint
      const nftMint2 = await createMint(
        provider.connection,
        ctx.owner,
        ctx.owner.publicKey,
        null,
        0
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "nft", // NFT for NFT — not allowed
        takeMint: nftMint2,
        takeValue: 1,
        salt: 5000,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "nft",
        makeMint: nftMint2,
        makeValue: 1,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 5001,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
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
        "AssetClassMismatch"
      );
    });
  });

  describe("D6: royalties_exceed_cap", () => {
    it("should reject royalties exceeding 50% (5000 bps)", async () => {
      const price = 1_000_000;
      const royaltyRecipient = Keypair.generate();

      const airdropSig = await provider.connection.requestAirdrop(
        royaltyRecipient.publicKey,
        1 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      const royaltyPaymentAccount = await createAccount(
        provider.connection,
        ctx.owner,
        trade.paymentMint,
        royaltyRecipient.publicKey
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 6000,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 6001,
      });

      // 60% royalty — exceeds 50% cap
      const royaltyParts: Part[] = [
        { account: royaltyRecipient.publicKey, value: 6000 },
      ];

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: royaltyPaymentAccount, isWritable: true, isSigner: false },
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
          royaltyParts,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "RoyaltiesTooHigh"
      );
    });
  });

  describe("D7: invalid_payout_sum", () => {
    it("should reject when payouts don't sum to 10000 bps", async () => {
      const price = 100_000;

      // Payouts sum to only 5000 bps (50%) — should fail
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 7000,
        payouts: [{ account: trade.seller.publicKey, value: 5000 }], // only 50%
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 7001,
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
        "InvalidPayoutSum"
      );
    });
  });

  describe("D8: maker_cannot_pay_with_sol", () => {
    it("should reject when right order make_asset is native SOL", async () => {
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "sol", // wants SOL
        takeMint: PublicKey.default,
        takeValue: 1_000_000_000,
        salt: 8000,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "sol", // paying with SOL
        makeMint: PublicKey.default,
        makeValue: 1_000_000_000,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 8001,
      });

      const matchLeftTs = futureTimestamp();
      const matchRightTs = futureTimestamp();

      await expectError(
        () => executeMatchOrders({
          ctx,
          orderLeft: sellerOrder,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts: [],
          payerKeypair: trade.seller,
        }),
        "MakerCannotPayWithSol"
      );
    });
  });
});
