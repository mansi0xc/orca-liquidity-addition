import { createHash } from 'crypto';
import { PublicKey } from '@solana/web3.js';
import { AssetType, Order } from '../types/index';

function writeU64LE(value: bigint): Buffer {
    const buf = Buffer.alloc(8);
    buf.writeBigUInt64LE(value, 0);
    return buf;
}

function writeI64LE(value: bigint): Buffer {
    const buf = Buffer.alloc(8);
    buf.writeBigInt64LE(value, 0);
    return buf;
}

export function hashAssetType(assetType: AssetType): Buffer {
    const hasher = createHash('sha256');
    hasher.update(Buffer.from([assetType.assetClass]));
    hasher.update(assetType.mint.toBuffer());
    hasher.update(writeU64LE(assetType.tokenId));
    return hasher.digest();
}

/**
 * Computes deterministic Order Key Hash (used as PDA seed for OrderFill).
 */
export function computeOrderKeyHash(order: Order): Buffer {
    const hasher = createHash('sha256');
    hasher.update(order.maker.toBuffer());
    hasher.update(hashAssetType(order.makeAsset.assetType));
    hasher.update(hashAssetType(order.takeAsset.assetType));
    hasher.update(writeU64LE(order.salt));
    hasher.update(Buffer.from([order.collectionBid ? 1 : 0]));
    return hasher.digest();
}

/**
 * Computes Order Hash (for verifying maker's signature).
 */
export function computeOrderHash(order: Order, programId: PublicKey): Buffer {
    const hasher = createHash('sha256');
    hasher.update(programId.toBuffer());
    hasher.update(Buffer.from('energi'));
    hasher.update(Buffer.from([1]));
    
    hasher.update(order.maker.toBuffer());
    hasher.update(hashAssetType(order.makeAsset.assetType));
    hasher.update(writeU64LE(order.makeAsset.value));
    
    hasher.update(order.taker.toBuffer());
    hasher.update(hashAssetType(order.takeAsset.assetType));
    hasher.update(writeU64LE(order.takeAsset.value));
    
    hasher.update(writeU64LE(order.salt));
    hasher.update(writeI64LE(order.start));
    hasher.update(writeI64LE(order.end));
    
    hasher.update(order.dataType);
    
    const dataHash = createHash('sha256').update(order.data).digest();
    hasher.update(dataHash);
    
    hasher.update(Buffer.from([order.collectionBid ? 1 : 0]));
    return hasher.digest();
}

/**
 * Computes Match Allowance Hash (signed by OrderBook API Backend).
 */
export function computeMatchAllowanceHash(orderKeyHash: Buffer, matchBeforeTimestamp: bigint, programId: PublicKey): Buffer {
    const hasher = createHash('sha256');
    hasher.update(programId.toBuffer());
    hasher.update(Buffer.from('energi'));
    hasher.update(Buffer.from([1]));
    hasher.update(orderKeyHash);
    hasher.update(writeI64LE(matchBeforeTimestamp));
    return hasher.digest();
}
