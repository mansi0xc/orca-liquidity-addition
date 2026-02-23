/**
 * ============================================================================
 * ORACLE API ROUTES
 * ============================================================================
 * 
 * REST API endpoints for the Whirlpool Oracle service.
 * 
 * ENDPOINTS:
 * ──────────
 * GET /oracle/:whirlpool         - Full oracle data (whirlpool + positions + aggregates)
 * GET /oracle/:whirlpool/price   - Just price data
 * GET /oracle/:whirlpool/positions - Position valuations
 * GET /health                    - Health check
 * 
 * QUERY PARAMETERS:
 * ─────────────────
 * - decimalsA: Override token A decimals
 * - decimalsB: Override token B decimals
 * - fullRangeOnly: Filter to only full-range positions (for /positions)
 */

import { FastifyInstance, FastifyRequest, FastifyReply } from "fastify";
import { Connection, PublicKey } from "@solana/web3.js";
import { OracleService } from "../services/oracleService";

/**
 * Query parameters for oracle endpoints
 */
interface OracleQueryParams {
  decimalsA?: string;
  decimalsB?: string;
  fullRangeOnly?: string;
}

/**
 * Route parameters with whirlpool address
 */
interface WhirlpoolParams {
  whirlpool: string;
}

/**
 * Register oracle routes with Fastify instance.
 * 
 * @param app - Fastify instance
 * @param connection - Solana connection
 */
export async function registerOracleRoutes(
  app: FastifyInstance,
  connection: Connection
): Promise<void> {
  const oracleService = new OracleService(connection);

  /**
   * Health check endpoint
   */
  app.get("/health", async (_request: FastifyRequest, reply: FastifyReply) => {
    try {
      const slot = await connection.getSlot();
      return reply.send({
        status: "healthy",
        slot,
        timestamp: Date.now(),
      });
    } catch (error) {
      return reply.status(503).send({
        status: "unhealthy",
        error: error instanceof Error ? error.message : "Unknown error",
        timestamp: Date.now(),
      });
    }
  });

  /**
   * GET /oracle/:whirlpool
   * 
   * Returns complete oracle data including:
   * - Whirlpool state
   * - Current price (raw and adjusted)
   * - All positions with valuations
   * - Aggregated metrics (TVL, total liquidity, distributions)
   */
  app.get<{
    Params: WhirlpoolParams;
    Querystring: OracleQueryParams;
  }>("/oracle/:whirlpool", async (request, reply) => {
    const { whirlpool } = request.params;
    const { decimalsA, decimalsB } = request.query;

    // Validate whirlpool address
    try {
      new PublicKey(whirlpool);
    } catch {
      return reply.status(400).send({
        error: "Invalid whirlpool address",
        message: "The provided address is not a valid Solana public key",
      });
    }

    try {
      const data = await oracleService.getOracleData(
        whirlpool,
        decimalsA ? parseInt(decimalsA) : undefined,
        decimalsB ? parseInt(decimalsB) : undefined
      );

      return reply.send(data);
    } catch (error) {
      console.error("Error fetching oracle data:", error);
      return reply.status(500).send({
        error: "Failed to fetch oracle data",
        message: error instanceof Error ? error.message : "Unknown error",
      });
    }
  });

  /**
   * GET /oracle/:whirlpool/price
   * 
   * Returns price data only:
   * - Raw price (tokenB per tokenA in smallest units)
   * - Adjusted price (human-readable)
   * - Inverse prices
   * - sqrtPrice components
   * - Current tick
   */
  app.get<{
    Params: WhirlpoolParams;
    Querystring: OracleQueryParams;
  }>("/oracle/:whirlpool/price", async (request, reply) => {
    const { whirlpool } = request.params;
    const { decimalsA, decimalsB } = request.query;

    // Validate whirlpool address
    try {
      new PublicKey(whirlpool);
    } catch {
      return reply.status(400).send({
        error: "Invalid whirlpool address",
        message: "The provided address is not a valid Solana public key",
      });
    }

    try {
      const priceData = await oracleService.getPrice(
        whirlpool,
        decimalsA ? parseInt(decimalsA) : undefined,
        decimalsB ? parseInt(decimalsB) : undefined
      );

      return reply.send({
        timestamp: Date.now(),
        whirlpool,
        ...priceData,
      });
    } catch (error) {
      console.error("Error fetching price data:", error);
      return reply.status(500).send({
        error: "Failed to fetch price data",
        message: error instanceof Error ? error.message : "Unknown error",
      });
    }
  });

  /**
   * GET /oracle/:whirlpool/positions
   * 
   * Returns position valuations:
   * - Each position's token amounts
   * - Liquidity and share percentage
   * - Total value in tokenB terms
   * - Tick bounds and full-range status
   * - Fees owed
   */
  app.get<{
    Params: WhirlpoolParams;
    Querystring: OracleQueryParams;
  }>("/oracle/:whirlpool/positions", async (request, reply) => {
    const { whirlpool } = request.params;
    const { fullRangeOnly } = request.query;

    // Validate whirlpool address
    try {
      new PublicKey(whirlpool);
    } catch {
      return reply.status(400).send({
        error: "Invalid whirlpool address",
        message: "The provided address is not a valid Solana public key",
      });
    }

    try {
      const positions = await oracleService.getPositions(
        whirlpool,
        fullRangeOnly === "true"
      );

      return reply.send({
        timestamp: Date.now(),
        whirlpool,
        count: positions.length,
        positions,
      });
    } catch (error) {
      console.error("Error fetching positions:", error);
      return reply.status(500).send({
        error: "Failed to fetch positions",
        message: error instanceof Error ? error.message : "Unknown error",
      });
    }
  });

  /**
   * GET /
   * 
   * API documentation / info
   */
  app.get("/", async (_request: FastifyRequest, reply: FastifyReply) => {
    return reply.send({
      name: "Orca Whirlpool Oracle API",
      version: "1.0.0",
      description: "Production-ready oracle for Orca Whirlpool full-range positions",
      endpoints: {
        "GET /health": "Health check",
        "GET /oracle/:whirlpool": "Full oracle data (whirlpool + positions + aggregates)",
        "GET /oracle/:whirlpool/price": "Price data only",
        "GET /oracle/:whirlpool/positions": "Position valuations",
      },
      queryParams: {
        decimalsA: "Override token A decimals (number)",
        decimalsB: "Override token B decimals (number)",
        fullRangeOnly: "Filter to full-range positions only (true/false)",
      },
      exampleWhirlpool: "8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN",
      math: {
        description: "Full-range liquidity math",
        formulas: {
          sqrtPrice: "sqrtPriceX64 / 2^64",
          price: "(sqrtPriceX64 / 2^64)^2",
          amountA: "liquidity / sqrtPrice",
          amountB: "liquidity * sqrtPrice",
        },
        note: "Full-range positions behave like constant-product AMM (x*y=k)",
      },
    });
  });
}
