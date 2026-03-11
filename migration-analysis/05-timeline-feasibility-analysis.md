# Timeline Feasibility Analysis

## 1. Hard Constraints

| Deliverable | Allocated | Hard Deadline |
|------------|-----------|---------------|
| **Mainnet Deployment** | 5 days | First week of April 2026 |
| **Full Stack Integration** | 14 days | First week of April 2026 |

These two tracks can run partially in parallel — full-stack integration can begin as soon as programs are deployed to devnet (before mainnet deployment). The mainnet deployment is the final 5-day step.

---

## 2. Mainnet Deployment (5 Days) — Feasibility

### Verdict: FEASIBLE — if prerequisites are met

The 5-day mainnet deployment window is realistic for deploying pre-built, audited programs and initializing protocol state. The day-by-day plan covers:

| Day | Activity | Confidence |
|-----|----------|------------|
| Day 1 | Deploy 6 programs, transfer authorities | High |
| Day 2 | Create Token-2022 mints, initialize protocol state | High |
| Day 3 | Oracle config, Whirlpool integration, LP Bonds config | Medium-High |
| Day 4 | Smoke tests, security validation | Medium |
| Day 5 | Backend deployment, go-live, monitoring | Medium |

**Confidence: 75%** that all 5 days execute cleanly.

**Primary risks:**
- Smoke test failures on Day 4 could push go-live to Day 6 (1-day slip)
- Orca Whirlpool CPI issues on mainnet (mitigated by devnet rehearsal)
- Token-2022 transfer hook unexpected behavior (mitigated by fallback plan)

**Prerequisites that MUST be complete before Day 1:**

| Prerequisite | Estimated Time | Status |
|-------------|---------------|--------|
| Token Authority + Transfer Hook programs developed | 2–3 weeks | Not started |
| Marketplace program developed | 4–5 weeks | Not started |
| Launchpad program developed | 2–3 weeks | Not started |
| All programs tested on devnet | 2 weeks | Not started |
| Security audit + remediation | 4–5 weeks | Not started |
| Squads multisig configured | 1 day | Not started |
| Helius RPC provisioned | 1 day | Not started |
| Deployment scripts rehearsed on devnet | 2–3 days | Not started |

**Critical path:** Program development (5 weeks parallel) → Testing (2 weeks) → Audit (4 weeks) → Deployment (5 days) = **~12 weeks minimum lead time** before the 5-day window.

If the hard deadline is first week of April and today is March 9, **there are only ~4 weeks remaining.** This means:

- Programs must be **already in development or nearly complete** to meet the deadline
- A full security audit (~4 weeks) cannot fit within 4 weeks alongside remaining development
- **Trade-off decision required:** either (a) defer audit and launch with internal review only (high risk), or (b) slip the deadline, or (c) reduce scope to already-migrated programs only (LP Bonds + Oracle)

---

## 3. Full Stack Integration (14 Days) — Feasibility

### Verdict: TIGHT BUT FEASIBLE — with 4 engineers and disciplined execution

The 14-day plan compresses 21 days of work into 14 by:
- Maximum parallelization across 4 engineers
- Scope reduction (launchpad frontend is simplified)
- Combined testing + deployment phases
- Aggressive daily deliverables

**Serial work estimate:** ~56 engineer-days across all tasks
**With 4 engineers:** ~14 days (56 / 4 = 14) — zero slack

| Phase | Days | Activities |
|-------|------|-----------|
| Foundation + APIs | Days 1–4 | Infrastructure, API migrations, order book start |
| Indexers + Frontend | Days 5–7 | Indexers, trading UI, evolution/redemption |
| Hardening + Launchpad | Days 8–9 | Service hardening, launchpad frontend |
| Testing | Days 10–11 | E2E tests, bug fixes, monitoring |
| Deployment | Days 12–14 | Staging, production, go-live |

**Confidence: 55%** that 14 days are sufficient without schedule slips.

**Primary risks:**
- Integration bugs discovered on Days 10–11 may need more than 2 days to fix
- LP Bonds API refactoring (SolanaLockerService + Borsh deserialization) is the most complex backend task — if it takes 6 days instead of 4, the entire schedule shifts
- Frontend transaction builders depend on stable Anchor IDLs — any program changes during integration force rework

### Minimum Viable Scope (if behind schedule)

If the team falls behind by Day 7, cut to minimum viable:

| Include | Exclude (defer) |
|---------|-----------------|
| LP Bonds API Solana support | Launchpad frontend |
| Oracle integration (already done) | Volume indexer |
| NFT indexer (core) | Metadata enrichment |
| Order book (create/query/cancel) | Evolution UI |
| Frontend: wallet + create bond + portfolio | Full marketplace trading UI |

This **minimum viable launch** is achievable in **10 days with 3 engineers** and still delivers LP bonds on Solana.

---

## 4. Combined Timeline (Counting Back from April Deadline)

If first week of April = April 6, 2026 (Monday), and today = March 9, 2026:

**Available working days:** ~20 working days (March 9 – April 3)

### Scenario A: Programs Already Built

If programs are complete and on devnet:

```
Mar 9  – Mar 10:  Prep deployment scripts, rehearse on devnet (2 days)
Mar 11 – Mar 28:  Full Stack Integration (14 working days)
Mar 29 – Apr 4:   Mainnet Deployment (5 working days, includes weekend work)
Apr 6:            Go-live Monday
```

**Verdict: FEASIBLE.** Integration and deployment fit within 20 days with 1 day buffer.

### Scenario B: Programs Still In Development

If programs are NOT complete:

```
Mar 9  – Mar 21:  Finish program development (2 weeks, assumes near-complete)
Mar 22 – Mar 28:  Devnet testing + Internal security review (1 week)
Mar 11 – Mar 28:  Full Stack Integration (overlapping with dev, using devnet IDLs)
Mar 29 – Apr 4:   Mainnet Deployment (5 days)
Apr 6:            Go-live Monday
```

**Verdict: EXTREMELY TIGHT.** Requires:
- Program development completes by March 21
- Full-stack integration starts Day 1 using devnet IDLs (risk of rework if IDLs change)
- No external security audit (only internal review)
- Zero schedule slips

### Scenario C: Programs Not Started

**Verdict: IMPOSSIBLE** for first week of April. Minimum 12 weeks needed from scratch. Earliest realistic date: **early June 2026**.

---

## 5. Parallel Execution Strategy

To maximize the 4-week window, run tracks in parallel:

```
WEEK 1 (Mar 9-14):
  Track A (Program Eng):  Finish program dev + devnet deploy
  Track B (Full Stack):   Day 1-5 of integration (foundation, APIs, order book, indexers)
                          ↳ Uses devnet program IDs for development

WEEK 2 (Mar 16-21):
  Track A:                Program testing + internal security review
  Track B:                Day 6-10 of integration (frontend, testing)
                          ↳ Program IDL may change — risk of rework

WEEK 3 (Mar 23-28):
  Track A:                Program freeze + deployment script prep
  Track B:                Day 11-14 of integration (monitoring, staging, production deploy)

WEEK 4 (Mar 30 - Apr 4):
  Track A + B merged:     Mainnet Deployment (5 days)

Apr 6: Go-live
```

This parallel strategy is the **only way** to meet the April deadline if programs aren't complete today.

---

## 6. Resource Requirements

### Minimum Team (14-day integration)

| Role | Count | Focus |
|------|-------|-------|
| Backend Engineer (Senior) | 1 | LP Bonds API, Rewards Service, User/General API |
| Backend Engineer | 1 | Solana Order Book, General API wiring |
| Indexer Engineer | 1 | NFT Indexer, Volume Indexer, Metadata Indexer |
| Frontend Engineer | 1 | Wallet integration, tx builders, UI, launchpad |
| **Total** | **4** | |

### For Mainnet Deployment (5 days)

| Role | Count | Focus |
|------|-------|-------|
| Solana Program Engineer | 1–2 | Program deployment, PDA init, authority transfer |
| Backend/DevOps | 1 | Service deployment, monitoring, RPC config |
| **Total** | **2–3** | |

### Overlap

Backend engineers from the integration track can assist with mainnet deployment (Day 5 of deployment = backend service deployment).

---

## 7. Key Decision Points

### Decision 1: Audit or No Audit? (Must decide by March 10)

| Option | Impact | Timeline Impact |
|--------|--------|----------------|
| Full external audit | High security confidence | +4 weeks → misses April deadline |
| Abbreviated audit (1 week, automated tools + focused review) | Medium security confidence | +1 week → tight but possible |
| Internal review only | Lower security confidence | No timeline impact |
| Launch LP Bonds only (already audited?), defer marketplace/launchpad | High security for launched scope | No timeline impact |

**Recommendation:** If the LP Bonds contracts are already audited, launch LP Bonds on Solana first. Defer marketplace and launchpad to a second phase with proper audit. This is the safest path that meets the April deadline.

### Decision 2: Scope — Full or Phased? (Must decide by March 10)

| Option | Scope | Feasibility |
|--------|-------|------------|
| **Phase 1 (April):** LP Bonds + Trading on Solana | LP Bonds API, Oracle, NFT Indexer, Order Book, Frontend (LP bonds) | **Feasible** |
| **Phase 2 (April + 2 weeks):** + Marketplace | Volume Indexer, full trading UI, stats | **Feasible** |
| **Phase 3 (April + 4 weeks):** + Launchpad | Launchpad program, frontend, metadata enrichment | **Feasible** |
| **Everything at once (April):** All features | All programs, all APIs, all indexers, all frontends | **High risk, low confidence** |

**Recommendation:** Phased approach. Ship Phase 1 by April. It delivers the core value (LP bonds on Solana) with acceptable risk. Phase 2 and 3 follow in fast-follow sprints.

### Decision 3: Integration Start Date (Must decide by March 10)

Full-stack integration should start **immediately** (March 10), using devnet program IDs. This gives the full 14 days before the deployment window.

If integration starts later than March 12, the 14-day plan must be compressed further or scope reduced.

---

## 8. Summary

| Question | Answer |
|----------|--------|
| Can mainnet deployment be done in 5 days? | **Yes**, if programs are built and audited |
| Can full-stack integration be done in 14 days? | **Yes**, with 4 engineers, disciplined execution, and minimal scope changes |
| Can everything fit before first week of April? | **Only if programs are near-complete today** and tracks run in parallel |
| What is the safest approach? | **Phase 1: LP Bonds only by April**, marketplace and launchpad in follow-up sprints |
| What is the biggest risk? | Attempting to ship everything at once without proper testing time |
