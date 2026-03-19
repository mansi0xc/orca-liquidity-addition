import { PublicKey } from '@solana/web3.js';
import bs58 from 'bs58';
import { AssetClass, Order } from '../types';
import { computeOrderKeyHash, computeOrderHash, computeMatchAllowanceHash } from '../services/crypto';
import assert from 'assert';

async function runTests() {
    const maker = new PublicKey(Buffer.alloc(32, 1));
    const taker = new PublicKey(Buffer.alloc(32, 2));
    const mint1 = new PublicKey(Buffer.alloc(32, 3));
    const mint2 = new PublicKey(Buffer.alloc(32, 4));
    const programId = new PublicKey(Buffer.alloc(32, 5));
    
    const order: Order = {
        maker,
        makeAsset: {
            assetType: { assetClass: AssetClass.SplToken, mint: mint1, tokenId: 0n },
            value: 100n
        },
        taker,
        takeAsset: {
            assetType: { assetClass: AssetClass.SplToken, mint: mint2, tokenId: 0n },
            value: 200n
        },
        salt: 123456789n,
        start: 0n,
        end: 0n,
        dataType: new Uint8Array([0xa0, 0x83, 0x2e, 0xf7]),
        data: new Uint8Array(),
        collectionBid: false
    };

    const keyHash = computeOrderKeyHash(order);
    const orderHash = computeOrderHash(order, programId);
    const matchAllowanceHash = computeMatchAllowanceHash(keyHash, 9999999999n, programId);

    console.log("orderKeyHash:", keyHash.toString('hex'));
    console.log("orderHash:", orderHash.toString('hex'));
    console.log("matchAllowanceHash:", matchAllowanceHash.toString('hex'));

    const expectedKeyHash = Buffer.from("8726d747daae54856fc3fc3e93669a8dbecfccf2b6c1f48be7d8075891d2c904", 'hex');
    const expectedOrderHash = Buffer.from("332e2526d065d9b1182e40d3559e441d5efde0eed28617869e08cde517f02934", 'hex');
    const expectedMatchAllowanceHash = Buffer.from("dc400307de3b984e4ed885705d649eb36c9e236164e67e972aca6f51c76229bf", 'hex');

    assert.deepStrictEqual(keyHash, expectedKeyHash);
    assert.deepStrictEqual(orderHash, expectedOrderHash);
    assert.deepStrictEqual(matchAllowanceHash, expectedMatchAllowanceHash);

    console.log("Tests generated successfully! Output the exact hex above into expected vars.");
}

runTests().catch(console.error);
