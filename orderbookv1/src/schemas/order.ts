import { z } from 'zod';
import { PublicKey } from '@solana/web3.js';
import bs58 from 'bs58';

const PublicKeySchema = z.string().refine((val) => {
    try {
        new PublicKey(val);
        return true;
    } catch {
        return false;
    }
}, { message: "Invalid PublicKey" });

const AssetTypeSchema = z.object({
    assetClass: z.number().int().min(0).max(4),
    mint: PublicKeySchema,
    tokenId: z.string().transform(val => BigInt(val))
});

const AssetSchema = z.object({
    assetType: AssetTypeSchema,
    value: z.string().transform(val => BigInt(val))
});

export const OrderSchema = z.object({
    maker: PublicKeySchema,
    makeAsset: AssetSchema,
    taker: PublicKeySchema,
    takeAsset: AssetSchema,
    salt: z.string().transform(val => BigInt(val)),
    start: z.string().transform(val => BigInt(val)),
    end: z.string().transform(val => BigInt(val)),
    dataType: z.array(z.number().min(0).max(255)).length(4).transform(val => new Uint8Array(val)),
    data: z.string().transform(val => bs58.decode(val)),
    collectionBid: z.boolean()
});
