import * as anchor from "@coral-xyz/anchor";
import { expect } from "chai";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { getAccount, createAccount, createMint, mintTo, approve } from "@solana/spl-token";

import {
  setupExchange,
  setupTradeAccounts,
  createOrder,
  executeMatchOrders,
  futureTimestamp,
  pastTimestamp,
  expectError,
  TestContext,
  TradeAccounts,
  Part,
  findAllowedTokenPDA,
  findOrderFillPDA,
  computeOrderKeyHash,
  buildSignatureInstructions,
  createEd25519Instruction,
  computeMatchAllowanceHash,
  computeOrderHash,
  DATA_TYPE_EMPTY,
} from "./helpers";

describe("Audit Test Suite", () => {
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

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 1: INVARIANT VERIFICATION TESTS
  // ═══════════════════════════════════════════════════════════════════

  describe("INV-1: Fungible <-> Non-Fungible Only", () => {
    it("should reject NFT <-> NFT trade", async () => {
      const otherNftMint = await createMint(
        provider.connection, ctx.owner, ctx.owner.publicKey, null, 0
      );
      const sellerOtherNft = await createAccount(
        provider.connection, ctx.owner, otherNftMint, trade.seller.publicKey
      );
      await mintTo(provider.connection, ctx.owner, otherNftMint, sellerOtherNft, ctx.owner, 1);

      const order1 = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "nft",
        takeMint: otherNftMint,
        takeValue: 1,
        salt: 100,
      });

      const order2 = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "nft",
        makeMint: otherNftMint,
        makeValue: 1,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 101,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: order1,
            orderRight: order2,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [],
            payerKeypair: trade.seller,
          }),
        "AssetClassMismatch"
      );
    });

    it("should reject SPL <-> SPL trade", async () => {
      const order1 = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: 1000,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 1000,
        salt: 102,
      });

      const order2 = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: 1000,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 1000,
        salt: 103,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: order1,
            orderRight: order2,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [],
            payerKeypair: trade.seller,
          }),
        "AssetClassMismatch"
      );
    });
  });

  describe("INV-5: Maker Cannot Pay with SOL", () => {
    it("should reject maker paying with native SOL", async () => {
      const order1 = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "sol",
        takeMint: PublicKey.default,
        takeValue: 1_000_000,
        salt: 104,
      });

      const order2 = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "sol",
        makeMint: PublicKey.default,
        makeValue: 1_000_000,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 105,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: order1,
            orderRight: order2,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [],
            payerKeypair: trade.seller,
          }),
        "MakerCannotPayWithSol"
      );
    });
  });

  describe("INV-4: Payout Sum Must Equal 10000", () => {
    it("should reject payouts summing to less than 10000", async () => {
      const order1 = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 1_000_000,
        salt: 106,
        payouts: [{ account: trade.seller.publicKey, value: 5000 }],
      });

      const order2 = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: 1_000_000,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 107,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: order1,
            orderRight: order2,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "InvalidPayoutSum"
      );
    });
  });

  describe("INV-3: Royalties Capped at 50%", () => {
    it("should reject royalties exceeding 5000 bps", async () => {
      const price = 1_000_000;
      const royaltyRecipient = Keypair.generate();
      const sig = await provider.connection.requestAirdrop(
        royaltyRecipient.publicKey, anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig);

      const royaltyAccount = await createAccount(
        provider.connection, ctx.owner, trade.paymentMint, royaltyRecipient.publicKey
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 108,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 109,
      });

      const royaltyParts: Part[] = [
        { account: royaltyRecipient.publicKey, value: 5001 },
      ];

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            royaltyParts,
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: royaltyAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "RoyaltiesTooHigh"
      );
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 2: SIGNATURE & AUTHENTICATION ATTACKS
  // ═══════════════════════════════════════════════════════════════════

  describe("SIG-1: Invalid Order Book Signature", () => {
    it("should reject when matchAllowance is signed by wrong key", async () => {
      const fakeOrderBook = Keypair.generate();
      const price = 500_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 200,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 201,
      });

      const fakeIxs = buildSignatureInstructions({
        orderLeft: sellerOrder,
        orderRight: buyerOrder,
        orderBookKeypair: fakeOrderBook,
        leftMakerKeypair: trade.seller,
        rightMakerKeypair: trade.buyer,
        matchLeftBeforeTimestamp: futureTimestamp(),
        matchRightBeforeTimestamp: futureTimestamp(),
        programId: ctx.program.programId,
        payer: trade.seller.publicKey,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
            sigIxOverride: fakeIxs,
          }),
        "InvalidSignature"
      );
    });
  });

  describe("SIG-2: Expired matchAllowance", () => {
    it("should reject expired matchAllowance timestamp", async () => {
      const price = 500_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 202,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 203,
      });

      const expiredTs = pastTimestamp(3600);

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: expiredTs,
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "MatchAllowanceExpired"
      );
    });
  });

  describe("SIG-3: Zero-Salt Maker Must Be Signer", () => {
    it("should reject when payer is not the zero-salt order maker", async () => {
      const price = 500_000;
      const impersonator = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        impersonator.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

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
        salt: 204,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: new anchor.BN(0),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: impersonator,
          }),
        "MakerMustBeSignerForZeroSalt"
      );
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 3: ACCOUNT SUBSTITUTION ATTACKS (HARDENING VERIFICATION)
  // ═══════════════════════════════════════════════════════════════════

  describe("ACCT-1: Fee Receiver Substitution (FIX C2 verification)", () => {
    it("should reject when fee receiver doesn't match config", async () => {
      const price = 1_000_000;
      const attacker = Keypair.generate();
      const attackerSig = await provider.connection.requestAirdrop(
        attacker.publicKey, anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(attackerSig);

      const attackerPaymentAccount = await createAccount(
        provider.connection, ctx.owner, trade.paymentMint, attacker.publicKey
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 300,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 301,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              // ATTACK: substitute fee receiver with attacker's account
              { pubkey: attackerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "InvalidRemainingAccounts"
      );
    });
  });

  describe("ACCT-2: Payout Destination Substitution (FIX C3 verification)", () => {
    it("should reject when payout destination doesn't match order data", async () => {
      const price = 1_000_000;
      const attacker = Keypair.generate();
      const attackerSig = await provider.connection.requestAirdrop(
        attacker.publicKey, anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(attackerSig);

      const attackerPaymentAccount = await createAccount(
        provider.connection, ctx.owner, trade.paymentMint, attacker.publicKey
      );

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 302,
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
        salt: 303,
        payouts: [{ account: trade.buyer.publicKey, value: 10000 }],
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              // ATTACK: substitute seller payout with attacker's account
              { pubkey: attackerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "InvalidRemainingAccounts"
      );
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 4: ORDER LIFECYCLE TESTS
  // ═══════════════════════════════════════════════════════════════════

  describe("CANCEL-1: Order Cancellation", () => {
    it("should cancel an order and prevent future matching", async () => {
      const price = 500_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 400,
      });

      const orderKeyHash = computeOrderKeyHash(sellerOrder);
      const [orderFillPDA] = findOrderFillPDA(orderKeyHash, program.programId);

      await program.methods
        .cancelOrder({
          orderKeyHash: Array.from(orderKeyHash),
          order: sellerOrder,
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
      expect(fill.fillAmount.toString()).to.equal(
        new anchor.BN("18446744073709551615").toString()
      );

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 401,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "OrderCancelled"
      );
    });
  });

  describe("CANCEL-2: Non-Maker Cannot Cancel", () => {
    it("should reject cancellation by non-maker", async () => {
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 500_000,
        salt: 402,
      });

      const orderKeyHash = computeOrderKeyHash(sellerOrder);
      const [orderFillPDA] = findOrderFillPDA(orderKeyHash, program.programId);

      await expectError(
        () =>
          program.methods
            .cancelOrder({
              orderKeyHash: Array.from(orderKeyHash),
              order: sellerOrder,
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
  });

  describe("CANCEL-3: Zero-Salt Cannot Be Cancelled", () => {
    it("should reject cancellation of zero-salt order", async () => {
      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: 500_000,
        salt: 0,
      });

      const orderKeyHash = computeOrderKeyHash(sellerOrder);
      const [orderFillPDA] = findOrderFillPDA(orderKeyHash, program.programId);

      await expectError(
        () =>
          program.methods
            .cancelOrder({
              orderKeyHash: Array.from(orderKeyHash),
              order: sellerOrder,
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
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 5: PAUSE MECHANISM TESTS
  // ═══════════════════════════════════════════════════════════════════

  describe("PAUSE-1: Paused Exchange Rejects Operations", () => {
    it("should reject match_orders when paused", async () => {
      await program.methods
        .togglePause()
        .accounts({
          exchangeConfig: ctx.exchangeConfig,
          owner: ctx.owner.publicKey,
        })
        .signers([ctx.owner])
        .rpc();

      const price = 500_000;
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

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
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
        .signers([ctx.owner])
        .rpc();
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 6: COLLECTION BID REJECTION
  // ═══════════════════════════════════════════════════════════════════

  describe("CBID-1: Collection Bids Rejected in match_orders", () => {
    it("should reject orders with collectionBid=true", async () => {
      const price = 500_000;

      const collectionBidOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 600,
        collectionBid: true,
      });

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 601,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: collectionBidOrder,
            orderRight: sellerOrder,
            leftMakerKeypair: trade.buyer,
            rightMakerKeypair: trade.seller,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [],
            payerKeypair: trade.buyer,
          }),
        "CollectionBidMustUseCollectionBidInstruction"
      );
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 7: COUNTERPARTY VALIDATION
  // ═══════════════════════════════════════════════════════════════════

  describe("CP-1: Counterparty Mismatch Rejection", () => {
    it("should reject when left.taker doesn't match right.maker", async () => {
      const price = 500_000;
      const wrongCounterparty = Keypair.generate();

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        taker: wrongCounterparty.publicKey,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 700,
      });

      const buyerOrder = createOrder({
        maker: trade.buyer.publicKey,
        makeAssetClass: "splToken",
        makeMint: trade.paymentMint,
        makeValue: price,
        takeAssetClass: "nft",
        takeMint: trade.nftMint,
        takeValue: 1,
        salt: 701,
      });

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [
              { pubkey: trade.buyerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.feeReceiverPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerPaymentAccount, isWritable: true, isSigner: false },
              { pubkey: trade.sellerNftAccount, isWritable: true, isSigner: false },
              { pubkey: trade.buyerNftAccount, isWritable: true, isSigner: false },
            ],
            payerKeypair: trade.seller,
          }),
        "CounterpartyMismatch"
      );
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 8: EXPIRED ORDER TESTS
  // ═══════════════════════════════════════════════════════════════════

  describe("EXP-1: Expired Order Rejected", () => {
    it("should reject order past its end timestamp", async () => {
      const price = 500_000;

      const sellerOrder = createOrder({
        maker: trade.seller.publicKey,
        makeAssetClass: "nft",
        makeMint: trade.nftMint,
        makeValue: 1,
        takeAssetClass: "splToken",
        takeMint: trade.paymentMint,
        takeValue: price,
        salt: 800,
        end: Math.floor(Date.now() / 1000) - 100,
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

      await expectError(
        () =>
          executeMatchOrders({
            ctx,
            orderLeft: sellerOrder,
            orderRight: buyerOrder,
            leftMakerKeypair: trade.seller,
            rightMakerKeypair: trade.buyer,
            matchLeftBeforeTimestamp: futureTimestamp(),
            matchRightBeforeTimestamp: futureTimestamp(),
            remainingAccounts: [],
            payerKeypair: trade.seller,
          }),
        "OrderExpired"
      );
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 9: ECONOMIC INVARIANT TESTS
  // ═══════════════════════════════════════════════════════════════════

  describe("ECON-1: Value Conservation", () => {
    it("total outflows should equal total inflows", async () => {
      const price = 10_000_000;

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

      const buyerBalanceBefore = Number(
        (await getAccount(provider.connection, trade.buyerPaymentAccount)).amount
      );

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
        matchLeftBeforeTimestamp: futureTimestamp(),
        matchRightBeforeTimestamp: futureTimestamp(),
        remainingAccounts,
        payerKeypair: trade.seller,
      });

      const buyerBalanceAfter = Number(
        (await getAccount(provider.connection, trade.buyerPaymentAccount)).amount
      );
      const sellerBalance = Number(
        (await getAccount(provider.connection, trade.sellerPaymentAccount)).amount
      );
      const feeBalance = Number(
        (await getAccount(provider.connection, trade.feeReceiverPaymentAccount)).amount
      );

      const totalOut = buyerBalanceBefore - buyerBalanceAfter;
      const totalIn = sellerBalance + feeBalance;

      expect(totalOut).to.equal(totalIn);
      expect(totalOut).to.equal(price);
    });
  });

  // ═══════════════════════════════════════════════════════════════════
  //  SECTION 10: ADMIN AUTHORIZATION TESTS
  // ═══════════════════════════════════════════════════════════════════

  describe("ADMIN-1: Unauthorized Admin Rejected", () => {
    it("should reject protocol fee change from non-owner", async () => {
      const attacker = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        attacker.publicKey, anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      await expectError(
        () =>
          program.methods
            .setProtocolFeeBps(500)
            .accounts({
              exchangeConfig: ctx.exchangeConfig,
              exchangeOwner: attacker.publicKey,
            })
            .signers([attacker])
            .rpc(),
        "Unauthorized"
      );
    });

    it("should reject toggle pause from non-owner", async () => {
      const attacker = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        attacker.publicKey, anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);

      await expectError(
        () =>
          program.methods
            .togglePause()
            .accounts({
              exchangeConfig: ctx.exchangeConfig,
              owner: attacker.publicKey,
            })
            .signers([attacker])
            .rpc(),
        "Unauthorized"
      );
    });
  });
});
