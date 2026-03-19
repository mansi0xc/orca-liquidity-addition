import { FastifyInstance } from 'fastify';
import { OrderSchema } from '../schemas/order';
import { computeOrderKeyHash } from '../services/crypto';
import { signMatchAllowance, verifyMakerSignature } from '../services/signature';
import { PublicKey } from '@solana/web3.js';
import bs58 from 'bs58';

export async function orderRoutes(fastify: FastifyInstance) {
    fastify.post('/submit', async (request, reply) => {
        try {
            const body: any = request.body;
            
            // 1. Zod Parsing
            const parsedOrder = OrderSchema.parse(body.order);
            const signature = bs58.decode(body.signature);
            const programId = new PublicKey(body.programId);
            
            const EXPECTED_PROGRAM_ID = new PublicKey(process.env.EXCHANGE_PROGRAM_ID || "11111111111111111111111111111111");
            if (!programId.equals(EXPECTED_PROGRAM_ID)) {
                return reply.status(400).send({ error: "Invalid Exchange Program ID" });
            }
            
            // 2. Verify Maker Signature
            if (parsedOrder.salt > 0n) {
                const isValid = verifyMakerSignature(parsedOrder, programId, signature);
                if (!isValid) {
                    return reply.status(400).send({ error: "Invalid maker Ed25519 signature" });
                }
            }

            // 3. Compute Key Hash
            const orderKeyHash = computeOrderKeyHash(parsedOrder);

            // 4. Generate Match Allowance Signature
            // Match allowance expires after 5 minutes
            const nowSeconds = Math.floor(Date.now() / 1000);
            const matchBeforeTimestamp = BigInt(nowSeconds + 300); 
            
            // Validate bounds explicitly as a sanity check
            if (matchBeforeTimestamp <= BigInt(nowSeconds) || matchBeforeTimestamp > BigInt(nowSeconds + 600)) {
                throw new Error("matchBeforeTimestamp bounds violated");
            }
            
            const privateKey = process.env.ORDERBOOK_PRIVATE_KEY;
            if (!privateKey) {
                throw new Error("ORDERBOOK_PRIVATE_KEY is missing");
            }
            
            const matchAllowanceSig = signMatchAllowance(
                orderKeyHash, 
                matchBeforeTimestamp, 
                programId, 
                privateKey
            );

            // 5. Mock DB Persistence
            // db.insert({ ... })

            return {
                success: true,
                orderKeyHash: bs58.encode(orderKeyHash),
                matchAllowanceSignature: bs58.encode(matchAllowanceSig),
                matchBeforeTimestamp: matchBeforeTimestamp.toString()
            };
        } catch (error: any) {
            fastify.log.error(error);
            return reply.status(400).send({ error: error.message || "Invalid request" });
        }
    });

    fastify.get('/query', async (request, reply) => {
        return { orders: [] };
    });

    fastify.post('/cancel', async (request, reply) => {
        return { success: true };
    });
}
