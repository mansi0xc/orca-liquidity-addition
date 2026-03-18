import * as anchor from "@coral-xyz/anchor";
import { expect } from "chai";
import {
  Keypair,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";

import {
  setupExchange,
  setupTradeAccounts,
  createOrder,
  computeOrderKeyHash,
  findOrderFillPDA,
  executeMatchOrders,
  futureTimestamp,
  expectError,
  TestContext,
  TradeAccounts,
} from "./helpers";

describe("Exchange - Cancel Orders", () => {
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

  describe("A2: test_cancel_order", () => {
    it("should cancel an order by setting fill to u64::MAX", async () => {
      const order = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 1_000_000,
        salt: 100,
      });

      const keyHash = computeOrderKeyHash(order);
      const [orderFillPDA] = findOrderFillPDA(keyHash, program.programId);

      await program.methods
        .cancelOrder({
          orderKeyHash: Array.from(keyHash),
          order,
        })
        .accounts({
          exchangeConfig: ctx.exchangeConfig,
          maker: trade.seller.publicKey,
          orderFill: orderFillPDA,
          payer: trade.seller.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([trade.seller])
        .rpc();

      const fill = await program.account.orderFill.fetch(orderFillPDA);
      expect(fill.fillAmount.toString()).to.equal(new anchor.BN("18446744073709551615").toString());
    });

    it("should prevent matching a cancelled order", async () => {
      const order = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 500_000,
        salt: 101,
      });

      const keyHash = computeOrderKeyHash(order);
      const [orderFillPDA] = findOrderFillPDA(keyHash, program.programId);

      // Cancel first
      await program.methods
        .cancelOrder({
          orderKeyHash: Array.from(keyHash),
          order,
        })
        .accounts({
          exchangeConfig: ctx.exchangeConfig,
          maker: trade.seller.publicKey,
          orderFill: orderFillPDA,
          payer: trade.seller.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([trade.seller])
        .rpc();

      // Now try to match — should fail
      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: 500_000,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 102,
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
          orderLeft: order,
          orderRight: buyerOrder,
          leftMakerKeypair: trade.seller,
          rightMakerKeypair: trade.buyer,
          matchLeftBeforeTimestamp: matchLeftTs,
          matchRightBeforeTimestamp: matchRightTs,
          remainingAccounts,
          payerKeypair: trade.seller,
        }),
        "OrderCancelled"
      );
    });
  });

  // ─── B. Negative Tests for Cancel ─────────────────────────────────

  describe("B: Cancel negative tests", () => {
    it("should reject cancel from non-maker", async () => {
      const order = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 100_000,
        salt: 200,
      });

      const keyHash = computeOrderKeyHash(order);
      const [orderFillPDA] = findOrderFillPDA(keyHash, program.programId);

      await expectError(
        () =>
          program.methods
            .cancelOrder({
              orderKeyHash: Array.from(keyHash),
              order,
            })
            .accounts({
              exchangeConfig: ctx.exchangeConfig,
              maker: trade.buyer.publicKey,
              orderFill: orderFillPDA,
              payer: trade.buyer.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([trade.buyer])
            .rpc(),
        "NotOrderMaker"
      );
    });

    it("should reject cancel for zero-salt order", async () => {
      const order = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 100_000,
        salt: 0,
      });

      const keyHash = computeOrderKeyHash(order);
      const [orderFillPDA] = findOrderFillPDA(keyHash, program.programId);

      await expectError(
        () =>
          program.methods
            .cancelOrder({
              orderKeyHash: Array.from(keyHash),
              order,
            })
            .accounts({
              exchangeConfig: ctx.exchangeConfig,
              maker: trade.seller.publicKey,
              orderFill: orderFillPDA,
              payer: trade.seller.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([trade.seller])
            .rpc(),
        "ZeroSaltCannotCancel"
      );
    });

    it("should reject cancel when exchange is paused", async () => {
      await program.methods
        .togglePause()
        .accounts({
          exchangeConfig: ctx.exchangeConfig,
          owner: ctx.owner.publicKey,
        })
        .rpc();

      const order = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 100_000,
        salt: 300,
      });

      const keyHash = computeOrderKeyHash(order);
      const [orderFillPDA] = findOrderFillPDA(keyHash, program.programId);

      await expectError(
        () =>
          program.methods
            .cancelOrder({
              orderKeyHash: Array.from(keyHash),
              order,
            })
            .accounts({
              exchangeConfig: ctx.exchangeConfig,
              maker: trade.seller.publicKey,
              orderFill: orderFillPDA,
              payer: trade.seller.publicKey,
              systemProgram: SystemProgram.programId,
            })
            .signers([trade.seller])
            .rpc(),
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
});
