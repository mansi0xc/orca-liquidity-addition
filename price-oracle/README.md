# Orca Whirlpool Oracle API

Production-ready TypeScript API oracle/indexer service for Orca Whirlpool **full-range** liquidity positions.

## Overview

This service provides real-time pricing, position valuations, and aggregate metrics for Orca Whirlpool positions. It is specifically designed for **full-range positions** where `tickLowerIndex = MIN_TICK` and `tickUpperIndex = MAX_TICK`.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                          API Server (Fastify)                       │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    Routes (routes/oracle.ts)                  │  │
│  │  GET /oracle/:whirlpool                                       │  │
│  │  GET /oracle/:whirlpool/price                                 │  │
│  │  GET /oracle/:whirlpool/positions                             │  │
│  └───────────────────────────────────────────────────────────────┘  │
│                              │                                      │
│  ┌───────────────────────────┴───────────────────────────────────┐  │
│  │                   OracleService (services/)                   │  │
│  │  Orchestrates: Whirlpool + Position + Math                    │  │
│  └───────────────────────────────────────────────────────────────┘  │
│          │                                    │                     │
│  ┌───────┴───────┐                    ┌───────┴───────┐            │
│  │WhirlpoolService│                    │PositionService│            │
│  │ - Fetch pool  │                    │ - Fetch all   │            │
│  │ - Decode      │                    │   positions   │            │
│  │ - Get vaults  │                    │ - Decode      │            │
│  └───────────────┘                    └───────────────┘            │
│                              │                                      │
│  ┌───────────────────────────┴───────────────────────────────────┐  │
│  │                    Math Utils (utils/math.ts)                 │  │
│  │  - Q64.64 math                                                │  │
│  │  - Full-range liquidity formulas                              │  │
│  │  - Price computations                                         │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                              │
                    ┌─────────┴─────────┐
                    │  Solana RPC Node  │
                    │  (Devnet/Mainnet) │
                    └───────────────────┘
```

## Mathematical Foundation

### Q64.64 Fixed-Point Format

Whirlpool stores `sqrtPrice` as a Q64.64 fixed-point number:
- 64 bits for the integer part
- 64 bits for the fractional part

To convert:
```
sqrtPrice = sqrtPriceX64 / 2^64
price = (sqrtPriceX64 / 2^64)^2
```

### Tick Index and Price Relationship

Each tick represents a discrete price point:
```
price = 1.0001^tick
tick = log(price) / log(1.0001)
```

- Tick 0 = price of 1
- Each tick = 0.01% price change (1 basis point)
- MIN_TICK = -443636 (price ≈ 0)
- MAX_TICK = 443636 (price ≈ ∞)

### Full-Range Liquidity Math

For concentrated liquidity (general case):
```
amountA = L * (1/sqrtP - 1/sqrtP_upper)  when P < P_upper
amountB = L * (sqrtP - sqrtP_lower)       when P > P_lower
```

For **full-range** positions (sqrtP_lower → 0, sqrtP_upper → ∞):
```
amountA = L / sqrtP
amountB = L * sqrtP
```

### Why Full-Range Behaves Like Constant Product

Full-range positions are mathematically equivalent to Uniswap V2 (x·y=k):

```
L² = amountA * amountB
   = (L / sqrtP) * (L * sqrtP)
   = L²  ✓
```

The position is **always in range** because the range is infinite, so it continuously provides liquidity at any price.

## Installation

```bash
# Clone or navigate to the price-oracle directory
cd orca-liquidity-addition/price-oracle

# Install dependencies
npm install

# Copy environment template
cp .env.example .env

# Edit .env with your RPC URL
```

## Configuration

Edit `.env`:

```env
# Solana RPC endpoint
RPC_URL=https://api.devnet.solana.com

# Server configuration
PORT=3000
HOST=0.0.0.0
```

## Usage

### Development

```bash
npm run dev
```

### Production

```bash
npm run build
npm start
```

## API Endpoints

### `GET /`
API information and documentation.

### `GET /health`
Health check endpoint.

**Response:**
```json
{
  "status": "healthy",
  "slot": 12345678,
  "timestamp": 1708789200000
}
```

### `GET /oracle/:whirlpool`
Complete oracle data including whirlpool state, all positions, and aggregates.

**Example:**
```bash
curl http://localhost:3000/oracle/8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN
```

**Query Parameters:**
- `decimalsA` - Override token A decimals
- `decimalsB` - Override token B decimals

**Response:**
```json
{
  "timestamp": 1708789200000,
  "whirlpool": {
    "address": "8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN",
    "sqrtPriceX64": "1844674407370955161600",
    "tickCurrentIndex": 0,
    "liquidity": "1000000000000",
    "tokenMintA": "So11111111111111111111111111111111111111112",
    "tokenMintB": "4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2",
    "tokenVaultA": "...",
    "tokenVaultB": "...",
    "vaultBalanceA": "1000000000",
    "vaultBalanceB": "2000000000",
    "tickSpacing": 64,
    "feeRate": 3000,
    "protocolFeeRate": 1000
  },
  "price": {
    "priceRaw": "1.000000000000000000",
    "price": "1.000000000000000000",
    "inversePriceRaw": "1.000000000000000000",
    "inversePrice": "1.000000000000000000",
    "sqrtPriceX64": "1844674407370955161600",
    "sqrtPrice": "1.000000000000000000",
    "tickCurrentIndex": 0
  },
  "tokenA": {
    "mint": "So11111111111111111111111111111111111111112",
    "decimals": 9
  },
  "tokenB": {
    "mint": "4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2",
    "decimals": 9
  },
  "positions": [
    {
      "positionAddress": "...",
      "positionMint": "...",
      "amountARaw": "500000000",
      "amountBRaw": "500000000",
      "amountA": "0.500000000",
      "amountB": "0.500000000",
      "liquidity": "500000000000",
      "liquidityShare": "50.0000",
      "totalValueInB": "1.000000000",
      "tickLowerIndex": -443584,
      "tickUpperIndex": 443584,
      "isFullRange": true,
      "feeOwedA": "0.000000000",
      "feeOwedB": "0.000000000"
    }
  ],
  "aggregate": {
    "totalPositions": 2,
    "fullRangePositions": 2,
    "totalLiquidity": "1000000000000",
    "totalAmountA": "1.000000000",
    "totalAmountB": "1.000000000",
    "totalAmountARaw": "1000000000",
    "totalAmountBRaw": "1000000000",
    "tvlInB": "2.000000000",
    "tvlInA": "2.000000000",
    "liquidityDistribution": [
      { "positionAddress": "...", "share": "50.0000" },
      { "positionAddress": "...", "share": "50.0000" }
    ]
  }
}
```

### `GET /oracle/:whirlpool/price`
Price data only.

**Response:**
```json
{
  "timestamp": 1708789200000,
  "whirlpool": "8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN",
  "priceRaw": "1.000000000000000000",
  "price": "1.000000000000000000",
  "inversePriceRaw": "1.000000000000000000",
  "inversePrice": "1.000000000000000000",
  "sqrtPriceX64": "1844674407370955161600",
  "sqrtPrice": "1.000000000000000000",
  "tickCurrentIndex": 0,
  "tokenMintA": "So11111111111111111111111111111111111111112",
  "tokenMintB": "4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2"
}
```

### `GET /oracle/:whirlpool/positions`
Position valuations only.

**Query Parameters:**
- `fullRangeOnly=true` - Filter to only full-range positions

**Response:**
```json
{
  "timestamp": 1708789200000,
  "whirlpool": "8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN",
  "count": 2,
  "positions": [...]
}
```

## Project Structure

```
price-oracle/
├── src/
│   ├── server.ts              # Entry point, Fastify server
│   ├── routes/
│   │   └── oracle.ts          # API route handlers
│   ├── services/
│   │   ├── index.ts           # Service exports
│   │   ├── whirlpoolService.ts # Fetch & decode whirlpool
│   │   ├── positionService.ts  # Fetch & decode positions
│   │   └── oracleService.ts    # Orchestration & valuation
│   ├── utils/
│   │   └── math.ts            # Q64.64 math, full-range formulas
│   └── types/
│       └── index.ts           # TypeScript interfaces
├── package.json
├── tsconfig.json
├── .env.example
└── README.md
```

## Technical Details

### Account Fetching

- **Whirlpool**: Single account fetch by address, decode with known offsets
- **Positions**: `getProgramAccounts` with memcmp filter on whirlpool field

### Decoding

Both services manually decode account data using buffer offsets:
- Skip 8-byte Anchor discriminator
- Read fields in order per Anchor struct layout

### Precision

- All core math uses `BigInt` or `BigNumber.js`
- No floating-point for price/amount calculations
- Q64.64 properly handled with 2^64 scaling

## Example Whirlpool

The default example whirlpool (devnet):
- Address: `8gbgyrnZJKiiUT29SJJ3VeJ7x7zHy11exABgD3omwVmN`
- Token A: wSOL (`So11111111111111111111111111111111111111112`)
- Token B: SPL Token (`4qbX8Mtx8XNt6DeCL414z67Dj9DJircMoSNEuX18AMB2`)

## License

Apache-2.0
