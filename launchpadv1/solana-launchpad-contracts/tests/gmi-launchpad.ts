import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { GmiLaunchpad } from "../target/types/gmi_launchpad";
import { expect } from "chai";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID } from "@solana/spl-token";

describe("gmi-launchpad", () => {
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.GmiLaunchpad as Program<GmiLaunchpad>;

    const authority = provider.wallet;
    const collectionId = Array.from(Buffer.from(anchor.web3.Keypair.generate().publicKey.toBytes()).slice(0, 32));
    const operatorRegistry = anchor.web3.Keypair.generate().publicKey;

    let minter = anchor.web3.Keypair.generate();

    before(async () => {
        // Fund minter
        const tx = await provider.connection.requestAirdrop(minter.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL);
        await provider.connection.confirmTransaction(tx);
    });

    it("Initializes a Standard Collection", async () => {
        const params = {
            collectionId,
            collectionType: { standard: {} },
            hasOperatorFilter: false,
            operatorRegistry: operatorRegistry,
            maxMintSupply: new anchor.BN(100),
            mintPrice: new anchor.BN(1 * anchor.web3.LAMPORTS_PER_SOL),
            presaleMintPrice: new anchor.BN(0.5 * anchor.web3.LAMPORTS_PER_SOL),
            maxUserMintAmount: new anchor.BN(5),
            maxTxMintAmount: new anchor.BN(2),
            presaleMaxUserMintAmount: new anchor.BN(3),
            presaleMaxTxMintAmount: new anchor.BN(1),
            name: "GMI Launchpad Test",
            symbol: "GMI",
            baseUri: "https://example.com/metadata/",
        };

        const [collectionPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("collection"), Buffer.from(collectionId)],
            program.programId
        );

        const [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("vault"), collectionPda.toBuffer()],
            program.programId
        );

        await program.methods
            .initializeCollection(params)
            .rpc();

        const state = await program.account.collection.fetch(collectionPda);
        expect(state.name).to.equal("GMI Launchpad Test");
        expect(state.symbol).to.equal("GMI");
        expect(state.maxMintSupply.toNumber()).to.equal(100);
        expect(state.reservedNfts.toNumber()).to.equal(0); // Standard has 0 reserved
    });

    it("Configures public sale to active", async () => {
        const [collectionPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("collection"), Buffer.from(collectionId)],
            program.programId
        );

        // Make public sale active by toggling
        await program.methods
            .togglePublicsale()
            .accounts({
                collection: collectionPda,
            })
            .rpc();

        const state = await program.account.collection.fetch(collectionPda);
        expect(state.publicsaleActive).to.be.true;
    });

    it("Mints an NFT in public sale", async () => {
        const [collectionPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("collection"), Buffer.from(collectionId)],
            program.programId
        );

        const nftMint = anchor.web3.Keypair.generate();
        const nftTokenAccount = anchor.web3.Keypair.generate();

        const [mintCounterPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("mint_counter"), collectionPda.toBuffer(), minter.publicKey.toBuffer()],
            program.programId
        );

        const [tokenRecordPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("token_record"), collectionPda.toBuffer(), nftMint.publicKey.toBuffer()],
            program.programId
        );

        const [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("vault"), collectionPda.toBuffer()],
            program.programId
        );

        // Initial balances
        const initialAuthBalance = await provider.connection.getBalance(authority.publicKey);

        await program.methods
            .mintPublic(new anchor.BN(1))
            .accounts({
                collection: collectionPda,
                nftMint: nftMint.publicKey,
                nftTokenAccount: nftTokenAccount.publicKey,
                ownerAccount: authority.publicKey,
                minter: minter.publicKey,
            })
            .signers([minter, nftMint, nftTokenAccount])
            .rpc();

        // Check balances (Standard payment goes to authority)
        const finalAuthBalance = await provider.connection.getBalance(authority.publicKey);
        expect(finalAuthBalance).to.be.above(initialAuthBalance);

        // Check state updates
        const state = await program.account.collection.fetch(collectionPda);
        expect(state.mintedAmount.toNumber()).to.equal(1);
        expect(state.totalMints.toNumber()).to.equal(1);

        const tokenRecord = await program.account.tokenRecord.fetch(tokenRecordPda);
        expect(tokenRecord.tokenIndex.toNumber()).to.equal(1);
        expect(tokenRecord.refundPrice.toNumber()).to.equal(0); // Standard has 0 refund price
        expect(tokenRecord.isOwnerMint).to.be.false;
    });
});
