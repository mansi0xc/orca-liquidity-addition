import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { OperatorRegistry } from "../target/types/operator_registry";
import { expect } from "chai";

describe("operator-registry", () => {
    // Configure the client to use the local cluster.
    const provider = anchor.AnchorProvider.env();
    anchor.setProvider(provider);

    const program = anchor.workspace.OperatorRegistry as Program<OperatorRegistry>;

    let authority = provider.wallet;
    let fundReceiver = anchor.web3.Keypair.generate();
    let collectionAddr = anchor.web3.Keypair.generate().publicKey;
    let operatorAddr = anchor.web3.Keypair.generate().publicKey;

    before(async () => {
        // Airdrop some SOL to fundReceiver for rent (if ever needed)
        const tx = await provider.connection.requestAirdrop(fundReceiver.publicKey, anchor.web3.LAMPORTS_PER_SOL);
        await provider.connection.confirmTransaction(tx);
    });

    it("Initializes the operator registry", async () => {
        const [registryPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("operator_registry")],
            program.programId
        );

        const sharePercentageBps = new anchor.BN(200); // 2%

        await program.methods
            .initializeRegistry(fundReceiver.publicKey, sharePercentageBps)
            .accounts({
                registryState: registryPda,
                authority: authority.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .rpc();

        const state = await program.account.operatorRegistryState.fetch(registryPda);
        expect(state.authority.toBase58()).to.equal(authority.publicKey.toBase58());
        expect(state.fundReceiver.toBase58()).to.equal(fundReceiver.publicKey.toBase58());
        expect(state.sharePercentageBps.toNumber()).to.equal(200);
        expect(state.paused).to.be.false;
    });

    it("Adds a collection-specific operator whitelist", async () => {
        const [whitelistPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [
                Buffer.from("operator_whitelist"),
                collectionAddr.toBuffer(),
                operatorAddr.toBuffer()
            ],
            program.programId
        );

        await program.methods
            .addOperatorWhitelist()
            .accounts({
                whitelistEntry: whitelistPda,
                collection: collectionAddr,
                operator: operatorAddr,
                authority: authority.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .rpc();

        const entry = await program.account.operatorWhitelist.fetch(whitelistPda);
        expect(entry.collection.toBase58()).to.equal(collectionAddr.toBase58());
        expect(entry.operator.toBase58()).to.equal(operatorAddr.toBase58());
    });

    it("Adds a universal operator", async () => {
        const [universalPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("universal_operator"), operatorAddr.toBuffer()],
            program.programId
        );

        await program.methods
            .addUniversalOperator()
            .accounts({
                universalOperatorEntry: universalPda,
                operator: operatorAddr,
                authority: authority.publicKey,
                systemProgram: anchor.web3.SystemProgram.programId,
            })
            .rpc();

        const entry = await program.account.universalOperator.fetch(universalPda);
        expect(entry.operator.toBase58()).to.equal(operatorAddr.toBase58());
    });

    it("Toggles registry pause", async () => {
        const [registryPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("operator_registry")],
            program.programId
        );

        await program.methods
            .toggleRegistryPause()
            .accounts({
                registryState: registryPda,
                authority: authority.publicKey,
            })
            .rpc();

        let state = await program.account.operatorRegistryState.fetch(registryPda);
        expect(state.paused).to.be.true;

        await program.methods
            .toggleRegistryPause()
            .accounts({
                registryState: registryPda,
                authority: authority.publicKey,
            })
            .rpc();

        state = await program.account.operatorRegistryState.fetch(registryPda);
        expect(state.paused).to.be.false;
    });

    it("Removes a universal operator", async () => {
        const [universalPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [Buffer.from("universal_operator"), operatorAddr.toBuffer()],
            program.programId
        );

        await program.methods
            .removeUniversalOperator()
            .accounts({
                universalOperatorEntry: universalPda,
                operator: operatorAddr,
                authority: authority.publicKey,
            })
            .rpc();

        try {
            await program.account.universalOperator.fetch(universalPda);
            expect.fail("Account should not exist");
        } catch (e) {
            expect(e.message).to.include("Account does not exist");
        }
    });

    it("Removes a collection-specific operator whitelist", async () => {
        const [whitelistPda] = anchor.web3.PublicKey.findProgramAddressSync(
            [
                Buffer.from("operator_whitelist"),
                collectionAddr.toBuffer(),
                operatorAddr.toBuffer()
            ],
            program.programId
        );

        await program.methods
            .removeOperatorWhitelist()
            .accounts({
                whitelistEntry: whitelistPda,
                collection: collectionAddr,
                operator: operatorAddr,
                authority: authority.publicKey,
            })
            .rpc();

        try {
            await program.account.operatorWhitelist.fetch(whitelistPda);
            expect.fail("Account should not exist");
        } catch (e) {
            expect(e.message).to.include("Account does not exist");
        }
    });
});
