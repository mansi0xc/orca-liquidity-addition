import * as anchor from "@coral-xyz/anchor";
import { expect } from "chai";
import { Keypair, PublicKey } from "@solana/web3.js";
import { getAccount } from "@solana/spl-token";

import {
  setupExchange,
  setupTradeAccounts,
  createOrder,
  executeMatchOrders,
  futureTimestamp,
  TestContext,
  TradeAccounts,
  Part,
} from "./helpers";

describe("Exchange - Execute Orders", () => {
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

  // ─── A. Functional Tests ─────────────────────────────────────────

  describe("A1: test_execute_order_success", () => {
    it("should execute a basic NFT-for-SPL trade", async () => {
      const price = 1_000_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 1,
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
        salt: 2,
        payouts: [{ account: trade.buyer.publicKey, value: 10000 }],
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

      // Use seller as payer to reduce Ed25519 instructions (payer==leftMaker skips left order sig)
      const result = await executeMatchOrders({
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

      // Verify NFT transferred to buyer
      const buyerNft = await getAccount(provider.connection, trade.buyerNftAccount);
      expect(buyerNft.amount.toString()).to.equal("1");

      const sellerNft = await getAccount(provider.connection, trade.sellerNftAccount);
      expect(sellerNft.amount.toString()).to.equal("0");

      // Verify payment transferred to seller (minus protocol fee)
      const sellerPayment = await getAccount(provider.connection, trade.sellerPaymentAccount);
      const protocolFee = Math.floor(price * 250 / 10000);
      const expectedSellerAmount = price - protocolFee;
      expect(Number(sellerPayment.amount)).to.be.approximately(expectedSellerAmount, 1);

      // Verify protocol fee was collected
      const feeReceiverAccount = await getAccount(provider.connection, trade.feeReceiverPaymentAccount);
      expect(Number(feeReceiverAccount.amount)).to.equal(protocolFee);

      // Verify fill PDAs were updated
      const leftFill = await program.account.orderFill.fetch(result.leftFillPDA);
      expect(leftFill.fillAmount.toNumber()).to.equal(price);

      const rightFill = await program.account.orderFill.fetch(result.rightFillPDA);
      expect(rightFill.fillAmount.toNumber()).to.equal(1);
    });
  });

  describe("A2: test_execute_order_with_zero_salt", () => {
    it("should execute when payer is maker (zero salt, no sig required)", async () => {
      const price = 500_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 0,
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
        salt: 3,
        payouts: [{ account: trade.buyer.publicKey, value: 10000 }],
      });

      const matchRightTs = futureTimestamp();

      const remainingAccounts = [
        { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
        { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
        { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
      ];

      await executeMatchOrders({
        ctx,
        orderLeft: sellerOrder,
        orderRight: buyerOrder,
        leftMakerKeypair: trade.seller,
        rightMakerKeypair: trade.buyer,
        matchLeftBeforeTimestamp: new anchor.BN(0),
        matchRightBeforeTimestamp: matchRightTs,
        remainingAccounts,
        payerKeypair: trade.seller,
      });

      const buyerNft = await getAccount(provider.connection, trade.buyerNftAccount);
      expect(buyerNft.amount.toString()).to.equal("1");
    });
  });

  describe("A3: test_fee_distribution", () => {
    it("should correctly distribute protocol fees and royalties", async () => {
      const price = 10_000_000;
      const royaltyRecipient = Keypair.generate();

      const sig = await provider.connection.requestAirdrop(
        royaltyRecipient.publicKey,
        1 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig);

      const { createAccount: createTokenAccount } = await import("@solana/spl-token");
      const royaltyPaymentAccount = await createTokenAccount(
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
        salt: 10,
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
        salt: 11,
        payouts: [{ account: trade.buyer.publicKey, value: 10000 }],
      });

      const royaltyParts: Part[] = [
        { account: royaltyRecipient.publicKey, value: 500 },
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

      await executeMatchOrders({
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
      });

      const protocolFee = Math.floor(price * 250 / 10000);
      const royaltyAmount = Math.floor(price * 500 / 10000);

      const feeAccount = await getAccount(provider.connection, trade.feeReceiverPaymentAccount);
      expect(Number(feeAccount.amount)).to.equal(protocolFee);

      const royaltyAccount = await getAccount(provider.connection, royaltyPaymentAccount);
      expect(Number(royaltyAccount.amount)).to.equal(royaltyAmount);

      const sellerAccount = await getAccount(provider.connection, trade.sellerPaymentAccount);
      const expectedSeller = price - protocolFee - royaltyAmount;
      expect(Number(sellerAccount.amount)).to.be.approximately(expectedSeller, 1);
    });
  });

  describe("A4: test_counterparty_restriction", () => {
    it("should succeed when taker field matches the other maker", async () => {
      const price = 100_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        taker: trade.buyer.publicKey,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 20,
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
        salt: 21,
        payouts: [{ account: trade.buyer.publicKey, value: 10000 }],
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

      const buyerNft = await getAccount(provider.connection, trade.buyerNftAccount);
      expect(buyerNft.amount.toString()).to.equal("1");
    });
  });
});
