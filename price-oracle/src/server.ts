/**
 * ============================================================================
 * ORCA WHIRLPOOL ORACLE SERVER
 * ============================================================================
 * 
 * Production-ready API server for indexing and querying Orca Whirlpool
 * positions. Designed specifically for FULL-RANGE liquidity positions.
 * 
 * ARCHITECTURE:
 * ─────────────
 * ┌─────────────────────────────────────────────────────────────────────┐
 * │                          API Server (Fastify)                       │
 * │  ┌───────────────────────────────────────────────────────────────┐  │
 * │  │                    Routes (routes/oracle.ts)                  │  │
 * │  │  GET /oracle/:whirlpool                                       │  │
 * │  │  GET /oracle/:whirlpool/price                                 │  │
 * │  │  GET /oracle/:whirlpool/positions                             │  │
 * │  └───────────────────────────────────────────────────────────────┘  │
 * │                              │                                      │
 * │  ┌───────────────────────────┴───────────────────────────────────┐  │
 * │  │                   OracleService (services/)                   │  │
 * │  │  Orchestrates: Whirlpool + Position + Math                    │  │
 * │  └───────────────────────────────────────────────────────────────┘  │
 * │          │                                    │                     │
 * │  ┌───────┴───────┐                    ┌───────┴───────┐            │
 * │  │WhirlpoolService│                    │PositionService│            │
 * │  │ - Fetch pool  │                    │ - Fetch all   │            │
 * │  │ - Decode      │                    │   positions   │            │
 * │  │ - Get vaults  │                    │ - Decode      │            │
 * │  └───────────────┘                    └───────────────┘            │
 * │                              │                                      │
 * │  ┌───────────────────────────┴───────────────────────────────────┐  │
 * │  │                    Math Utils (utils/math.ts)                 │  │
 * │  │  - Q64.64 math                                                │  │
 * │  │  - Full-range liquidity: amountA = L/sqrtP, amountB = L*sqrtP │  │
 * │  │  - Price computations                                         │  │
 * │  └───────────────────────────────────────────────────────────────┘  │
 * └─────────────────────────────────────────────────────────────────────┘
 *                              │
 *                    ┌─────────┴─────────┐
 *                    │  Solana RPC Node  │
 *                    │  (Devnet/Mainnet) │
 *                    └───────────────────┘
 * 
 * MATH SUMMARY (FULL RANGE POSITIONS):
 * ────────────────────────────────────
 * Full-range positions span tickLower = MIN_TICK to tickUpper = MAX_TICK.
 * This means they're ALWAYS in range, regardless of current price.
 * 
 * The math simplifies dramatically:
 *   amountA = L / sqrtPrice
 *   amountB = L * sqrtPrice
 * 
 * Why? Because:
 * 1. General CLMM formula: amountA = L * (1/sqrtP - 1/sqrtPu) when P < Pu
 * 2. For full-range: sqrtPu → ∞, so 1/sqrtPu → 0
 * 3. Result: amountA = L / sqrtP
 * 
 * Similarly for amountB:
 * 1. General formula: amountB = L * (sqrtP - sqrtPl) when P > Pl
 * 2. For full-range: sqrtPl → 0
 * 3. Result: amountB = L * sqrtP
 * 
 * This makes full-range positions equivalent to classic Uniswap V2
 * constant-product positions: x * y = k (where k = L^2).
 * 
 * USAGE:
 * ──────
 * 1. Copy .env.example to .env
 * 2. Set RPC_URL (devnet or mainnet)
 * 3. npm install
 * 4. npm run dev (development) or npm start (production)
 * 
 * EXAMPLE REQUEST:
 * ────────────────
 * GET http://localhost:3000/oracle/8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN
 * 
 */

import Fastify from "fastify";
import { Connection } from "@solana/web3.js";
import dotenv from "dotenv";
import { registerOracleRoutes } from "./routes/oracle";

// Load environment variables
dotenv.config();

// Configuration
const config = {
  rpcUrl: process.env.RPC_URL || "https://api.devnet.solana.com",
  port: parseInt(process.env.PORT || "3000"),
  host: process.env.HOST || "0.0.0.0",
};

/**
 * Bootstrap and start the server.
 */
async function main() {
  console.log("═".repeat(70));
  console.log("ORCA WHIRLPOOL ORACLE SERVER");
  console.log("═".repeat(70));
  console.log();
  console.log("Configuration:");
  console.log(`  RPC URL: ${config.rpcUrl}`);
  console.log(`  Port: ${config.port}`);
  console.log(`  Host: ${config.host}`);
  console.log();

  // Create Solana connection
  const connection = new Connection(config.rpcUrl, "confirmed");
  
  // Verify connection
  try {
    const slot = await connection.getSlot();
    console.log(`Connected to Solana (slot: ${slot})`);
  } catch (error) {
    console.error("Failed to connect to Solana RPC:", error);
    process.exit(1);
  }

  // Create Fastify instance
  const app = Fastify({
    logger: {
      level: "info",
      transport: {
        target: "pino-pretty",
        options: {
          colorize: true,
          translateTime: "HH:MM:ss",
          ignore: "pid,hostname",
        },
      },
    },
  });

  // Register routes
  await registerOracleRoutes(app, connection);

  // Start server
  try {
    await app.listen({ port: config.port, host: config.host });
    console.log();
    console.log("═".repeat(70));
    console.log(`Server running at http://${config.host}:${config.port}`);
    console.log("═".repeat(70));
    console.log();
    console.log("Available endpoints:");
    console.log(`  GET /                                    - API info`);
    console.log(`  GET /health                              - Health check`);
    console.log(`  GET /oracle/:whirlpool                   - Full oracle data`);
    console.log(`  GET /oracle/:whirlpool/price             - Price data`);
    console.log(`  GET /oracle/:whirlpool/positions         - Position valuations`);
    console.log();
    console.log("Example:");
    console.log(`  curl http://localhost:${config.port}/oracle/8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN`);
    console.log();
  } catch (error) {
    console.error("Failed to start server:", error);
    process.exit(1);
  }

  // Graceful shutdown
  const signals = ["SIGINT", "SIGTERM"];
  signals.forEach((signal) => {
    process.on(signal, async () => {
      console.log(`\nReceived ${signal}, shutting down...`);
      await app.close();
      process.exit(0);
    });
  });
}

// Run
main().catch((error) => {
  console.error("Fatal error:", error);
  process.exit(1);
});
