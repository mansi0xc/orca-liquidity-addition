import nacl from 'tweetnacl';
import bs58 from 'bs58';
import { PublicKey } from '@solana/web3.js';
import { computeMatchAllowanceHash, computeOrderHash } from './crypto';
import { Order } from '../types';

export function signMatchAllowance(
    orderKeyHash: Buffer, 
    matchBeforeTimestamp: bigint, 
    programId: PublicKey, 
    privateKeyBase58: string
): Uint8Array {
    const message = computeMatchAllowanceHash(orderKeyHash, matchBeforeTimestamp, programId);
    const secretKey = bs58.decode(privateKeyBase58);
    
    if (secretKey.length !== 64) {
        throw new Error("Invalid Ed25519 private key length. Expected 64 bytes.");
    }
    
    return nacl.sign.detached(new Uint8Array(message), secretKey);
}

export function verifyMakerSignature(
    order: Order, 
    programId: PublicKey, 
    signature: Uint8Array
): boolean {
    const message = computeOrderHash(order, programId);
    return nacl.sign.detached.verify(
        new Uint8Array(message),
        signature,
        new Uint8Array(order.maker.toBuffer())
    );
}
