import { PublicKey } from "@solana/web3.js";

export enum AssetClass {
    Sol = 0,
    WrappedSol = 1,
    SplToken = 2,
    Nft = 3,
    SemiFungible = 4,
}

export interface AssetType {
    assetClass: AssetClass;
    mint: PublicKey;
    tokenId: bigint; // u64
}

export interface Asset {
    assetType: AssetType;
    value: bigint; // u64
}

export interface Order {
    maker: PublicKey;
    makeAsset: Asset;
    taker: PublicKey;
    takeAsset: Asset;
    salt: bigint; // u64
    start: bigint; // i64
    end: bigint; // i64
    dataType: Uint8Array;
    data: Uint8Array;
    collectionBid: boolean;
}
