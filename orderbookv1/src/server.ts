import Fastify from 'fastify';
import { orderRoutes } from './routes/index';
import dotenv from 'dotenv';
dotenv.config();

const server = Fastify({ logger: true });

server.register(orderRoutes, { prefix: '/v1/order' });

const start = async () => {
    try {
        await server.listen({ port: 3000, host: '0.0.0.0' });
        console.log('Solana Orderbook API listening on port 3000');
    } catch (err) {
        server.log.error(err);
        process.exit(1);
    }
};

start();
