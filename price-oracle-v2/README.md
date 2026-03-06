# Solana Price Oracle API

TypeScript oracle service for the Solana LP Bonds protocol. Reads on-chain Orca Whirlpool position data, computes token amounts, and produces Ed25519-signed messages consumed by the `lp_bonds` and `lp_bonds_evolution` programs.

## Behavioral Parity with EVM Oracle

This oracle is a protocol migration of the EVM LP Bond Amount Fetcher (`lp-bond-amount-fetcher/`). The table below maps every behavioral difference.

| Dimension | EVM Oracle | Solana Oracle |
|---|---|---|
| **Framework** | Express.js | Fastify |
| **AMM** | Uniswap V3 (sqrtPriceX96, Q96.96) | Orca Whirlpool (sqrtPriceX64, Q64.64) |
| **Signature** | ECDSA secp256k1 + Ethereum prefix | Ed25519 (tweetnacl, raw message) |
| **Nonce model** | Global (contract-level counter) | Per-user PDA (`["nonce", user]`) |
| **Message format** | `solidityPackedKeccak256(tokenId, amt0, amt1, contract, nonce, sender)` | Fixed 198-byte canonical: domain + bondMint + positionMint + amt0 + amt1 + liquidity + ticks + nonce + sender + contract |
| **Position amounts** | Uniswap V3 SDK `Position.amount0/amount1` | BigNumber.js reimplementation of concentrated liquidity math |
| **Bond identifier** | `tokenId` (uint256 ERC-721 ID) | `bondMint` (Solana public key) |
| **Contract address** | EVM contract address in signature | Program ID in signature |
| **Token decimals** | Hardcoded per chain config | Fetched from on-chain mint accounts |

### Preserved Guarantees
- `amount0` and `amount1` are computed with **floor rounding** (ROUND_DOWN)
- Signature covers ALL position data (amounts, ticks, liquidity, nonce, sender, contract)
- Nonce must be strictly increasing — replay protection enforced on-chain
- Zero-liquidity positions are excluded from oracle data
- Full-range detection uses aligned tick bounds

## Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/health` | Health check with block height and oracle pubkey |
| GET | `/oracle/:whirlpool` | Full oracle data (prices, positions, valuations) |
| GET | `/oracle/:whirlpool/price` | Price data only |
| GET | `/oracle/:whirlpool/positions` | Position valuations only |
| POST | `/position-info` | **Signed position info** for `verify_collateral` |
| POST | `/signed-positions` | Batch signed positions for a whirlpool |
| POST | `/evolution-info` | **Signed evolution data** for `evolve_bond` (NEW) |
| GET | `/generate-keypair` | Dev-only keypair generator |

### POST /position-info

```json
// Request
{
  "bondMint": "<base58 pubkey>",
  "sender": "<base58 pubkey>",
  "nonce": "1"
}

// Response
{
  "bondMint": "...",
  "positionMint": "...",
  "whirlpool": "...",
  "position": {
    "positionMint": "...",
    "liquidity": "...",
    "tickLowerIndex": -443636,
    "tickUpperIndex": 443636,
    "amount0": "...",
    "amount1": "...",
    "isFullRange": true
  },
  "tickCurrent": "...",
  "hasLiquidity": true,
  "liquidity": "...",
  "amount0": "...",
  "amount1": "...",
  "level": 1,
  "poolAddress": "...",
  "tickLower": -443636,
  "tickUpper": 443636,
  "fee": 3000,
  "oracleSignature": {
    "signature": "<base64>",
    "message": "<base64>",
    "publicKey": "<base58>",
    "messageHex": "<hex>"
  },
  "nonce": "1",
  "timestamp": "..."
}
```

### POST /evolution-info

```json
// Request
{
  "sourceBondMint": "<base58 pubkey>",
  "targetLevel": 2,
  "sender": "<base58 pubkey>",
  "nonce": "1"
}

// Response
{
  "sourceBondMint": "...",
  "targetLevel": 2,
  "amountA": "...",
  "amountB": "...",
  "liquidity": "...",
  "oracleSignature": { "..." },
  "nonce": "1",
  "timestamp": "..."
}
```

## Setup

```bash
npm install
cp .env.example .env
# Edit .env with your values
```

### Environment Variables

| Variable | Description |
|---|---|
| `RPC_URL` | Solana RPC endpoint |
| `ORACLE_PRIVATE_KEY` | Base64-encoded 64-byte Ed25519 secret key |
| `LP_BONDS_PROGRAM_ID` | LP Bonds program ID (for L1 signatures) |
| `LP_BONDS_EVOLUTION_PROGRAM_ID` | Evolution program ID (for L2-4 signatures) |
| `PORT` | Server port (default: 3000) |
| `HOST` | Bind address (default: 0.0.0.0) |

### Generate Keypair

```bash
# Dev mode only — GET /generate-keypair returns a new keypair
# Set the returned secretKey as ORACLE_PRIVATE_KEY
# Set the returned publicKey as oracle_authority on-chain
```

## Build & Run

```bash
# Development
npm run dev

# Production
npm run build
npm start
```

## Architecture

```
src/
├── server.ts                 # Fastify entry point
├── routes/
│   └── oracle.ts             # Route handlers + Zod validation
├── services/
│   ├── index.ts              # Barrel exports
│   ├── whirlpoolService.ts   # Decode Orca Whirlpool accounts (653 bytes)
│   ├── positionService.ts    # Decode Orca Position accounts (216 bytes)
│   ├── custodyService.ts     # Decode LP Bonds custody accounts (212 bytes)
│   ├── signatureService.ts   # Ed25519 signing + canonical message builders
│   └── oracleService.ts      # Main orchestrator
├── types/
│   └── index.ts              # All interfaces, constants, PDA seeds
└── utils/
    └── math.ts               # Position amount math (sqrtPriceX64)
```

## Canonical Message Formats

### L1 — verify_collateral (198 bytes)

| Offset | Field | Bytes | Type |
|---|---|---|---|
| 0 | `"LP_BONDS_SOLANA_V1"` | 18 | utf-8 |
| 18 | `bond_mint` | 32 | Pubkey |
| 50 | `position_mint` | 32 | Pubkey |
| 82 | `amount0` | 8 | u64 LE |
| 90 | `amount1` | 8 | u64 LE |
| 98 | `liquidity` | 16 | u128 LE |
| 114 | `tick_lower` | 4 | i32 LE |
| 118 | `tick_upper` | 4 | i32 LE |
| 122 | `tick_current` | 4 | i32 LE |
| 126 | `nonce` | 8 | u64 LE |
| 134 | `sender` | 32 | Pubkey |
| 166 | `contract_address` | 32 | Pubkey |

### Evolution — evolve_bond (155 bytes)

| Offset | Field | Bytes | Type |
|---|---|---|---|
| 0 | `"LP_BONDS_EVOLVE_V1"` | 18 | utf-8 |
| 18 | `source_bond_mint` | 32 | Pubkey |
| 50 | `target_level` | 1 | u8 |
| 51 | `amount_a` | 8 | u64 LE |
| 59 | `amount_b` | 8 | u64 LE |
| 67 | `liquidity` | 16 | u128 LE |
| 83 | `nonce` | 8 | u64 LE |
| 91 | `sender` | 32 | Pubkey |
| 123 | `contract_address` | 32 | Pubkey |

## Bug Fixes vs Prior Compiled Oracle

1. **CUSTODY_ACCOUNT_SIZE**: Fixed from 170 to 212 bytes. The prior version was missing `level`, `lock_duration`, `is_evolved`, `evolved_from`, `bump`, and `position_bump` fields.
2. **Evolution support**: Added `POST /evolution-info` endpoint and `buildEvolutionCanonicalMessage()` for the `lp_bonds_evolution` program.

## Add your files

- [ ] [Create](https://docs.gitlab.com/ee/user/project/repository/web_editor.html#create-a-file) or [upload](https://docs.gitlab.com/ee/user/project/repository/web_editor.html#upload-a-file) files
- [ ] [Add files using the command line](https://docs.gitlab.com/ee/gitlab-basics/add-file.html#add-a-file-using-the-command-line) or push an existing Git repository with the following command:

```
cd existing_repo
git remote add origin https://git.energi.software/energi/tech/dweb/nft/marketplace/api/solana-price-oracle.git
git branch -M main
git push -uf origin main
```

## Integrate with your tools

- [ ] [Set up project integrations](https://git.energi.software/energi/tech/dweb/nft/marketplace/api/solana-price-oracle/-/settings/integrations)

## Collaborate with your team

- [ ] [Invite team members and collaborators](https://docs.gitlab.com/ee/user/project/members/)
- [ ] [Create a new merge request](https://docs.gitlab.com/ee/user/project/merge_requests/creating_merge_requests.html)
- [ ] [Automatically close issues from merge requests](https://docs.gitlab.com/ee/user/project/issues/managing_issues.html#closing-issues-automatically)
- [ ] [Enable merge request approvals](https://docs.gitlab.com/ee/user/project/merge_requests/approvals/)
- [ ] [Automatically merge when pipeline succeeds](https://docs.gitlab.com/ee/user/project/merge_requests/merge_when_pipeline_succeeds.html)

## Test and Deploy

Use the built-in continuous integration in GitLab.

- [ ] [Get started with GitLab CI/CD](https://docs.gitlab.com/ee/ci/quick_start/index.html)
- [ ] [Analyze your code for known vulnerabilities with Static Application Security Testing(SAST)](https://docs.gitlab.com/ee/user/application_security/sast/)
- [ ] [Deploy to Kubernetes, Amazon EC2, or Amazon ECS using Auto Deploy](https://docs.gitlab.com/ee/topics/autodevops/requirements.html)
- [ ] [Use pull-based deployments for improved Kubernetes management](https://docs.gitlab.com/ee/user/clusters/agent/)
- [ ] [Set up protected environments](https://docs.gitlab.com/ee/ci/environments/protected_environments.html)

***

# Editing this README

When you're ready to make this README your own, just edit this file and use the handy template below (or feel free to structure it however you want - this is just a starting point!). Thank you to [makeareadme.com](https://www.makeareadme.com/) for this template.

## Suggestions for a good README
Every project is different, so consider which of these sections apply to yours. The sections used in the template are suggestions for most open source projects. Also keep in mind that while a README can be too long and detailed, too long is better than too short. If you think your README is too long, consider utilizing another form of documentation rather than cutting out information.

## Name
Choose a self-explaining name for your project.

## Description
Let people know what your project can do specifically. Provide context and add a link to any reference visitors might be unfamiliar with. A list of Features or a Background subsection can also be added here. If there are alternatives to your project, this is a good place to list differentiating factors.

## Badges
On some READMEs, you may see small images that convey metadata, such as whether or not all the tests are passing for the project. You can use Shields to add some to your README. Many services also have instructions for adding a badge.

## Visuals
Depending on what you are making, it can be a good idea to include screenshots or even a video (you'll frequently see GIFs rather than actual videos). Tools like ttygif can help, but check out Asciinema for a more sophisticated method.

## Installation
Within a particular ecosystem, there may be a common way of installing things, such as using Yarn, NuGet, or Homebrew. However, consider the possibility that whoever is reading your README is a novice and would like more guidance. Listing specific steps helps remove ambiguity and gets people to using your project as quickly as possible. If it only runs in a specific context like a particular programming language version or operating system or has dependencies that have to be installed manually, also add a Requirements subsection.

## Usage
Use examples liberally, and show the expected output if you can. It's helpful to have inline the smallest example of usage that you can demonstrate, while providing links to more sophisticated examples if they are too long to reasonably include in the README.

## Support
Tell people where they can go to for help. It can be any combination of an issue tracker, a chat room, an email address, etc.

## Roadmap
If you have ideas for releases in the future, it is a good idea to list them in the README.

## Contributing
State if you are open to contributions and what your requirements are for accepting them.

For people who want to make changes to your project, it's helpful to have some documentation on how to get started. Perhaps there is a script that they should run or some environment variables that they need to set. Make these steps explicit. These instructions could also be useful to your future self.

You can also document commands to lint the code or run tests. These steps help to ensure high code quality and reduce the likelihood that the changes inadvertently break something. Having instructions for running tests is especially helpful if it requires external setup, such as starting a Selenium server for testing in a browser.

## Authors and acknowledgment
Show your appreciation to those who have contributed to the project.

## License
For open source projects, say how it is licensed.

## Project status
If you have run out of energy or time for your project, put a note at the top of the README saying that development has slowed down or stopped completely. Someone may choose to fork your project or volunteer to step in as a maintainer or owner, allowing your project to keep going. You can also make an explicit request for maintainers.
