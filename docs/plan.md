# Fogline — product plan

Status: Phase 1 in progress, specified in [specs/phase-1.md](specs/phase-1.md) · Written 2026-11-15 · **Fogline** is a working name (§12)

Fogline is a ground-up redesign of [CodebaseQA](https://github.com/ShreeBohara/codebaseqa), called v1 below. This plan records why the product changed, what it is now, and the ten phases that build it.

External facts were checked on 2026-11-15 and are linked in §13. Anything marked *target* is a goal to be measured, not a result. Effort figures are estimates.

---

## 1. The short version

**Where v1 stood.** Its engineering habits were good; its product was in the wrong place. "Chat with a repo, plus AI-written lessons" is what free tools from Cognition and Google now do by default. Open source moved even faster: five code-graph projects created in 2026 passed 30k stars, led by Graphify (148k) and Understand-Anything (93k, graph plus guided tours). Rebuilding any of that, however well, produces a smaller copy.

**The decision.** Keep the domain (helping people understand code) and change the product. Everyone built tools that explain code or feed it to agents. Nobody closes the loop on whether a human actually understands it, at the moment agents are writing code faster than people can read it.

**What Fogline is, in one sentence.** An open-source, local-first tool that maps which parts of a codebase have evidence of human understanding and which do not, then closes the gap with exercises whose answers come from the repository's own tests and history, never from an LLM.

**The loop.**

1. **Measure** — who or what wrote each line, what evidence exists that a person understands it, and where the risky unknown regions are. Deterministic, zero LLM calls.
2. **Prove** — predict what a real test does, re-fix a real past bug, repair a seeded one. The repository's tests grade it. Passing lifts the fog on that region.
3. **Learn** — tours and a guide grounded in the real graph and git history, every statement labeled as checked fact or model narrative.

**What makes it worth building.** A point of view backed by current research, a map as the centerpiece, real systems work (an engine that tracks every line through history), measured retrieval with published benchmark numbers, an original empirical study, and a hosting bill near zero by design.

**Cost.** $1–6 per month: a domain plus an optional capped LLM quota.

**Shape.** Ten phases, three public releases. Phases 1–5 are the project; 6–10 are depth, and each stands alone.

---

## 2. What v1 was, and why it changed

| Area | What is there | Verdict |
|---|---|---|
| Core product | RAG chat with citations, persona learning tracks, quizzes, XP, import graph | Commoditized. Free tools do this, and agentic search beats top-6-chunk RAG on repository questions. |
| Retrieval | 6 profiles × 8 hand-tuned weights, substring keyword matching, LLM rerank of the top 18 | Never measured. The eval set is one case. This is the largest technical gap. |
| Understanding model | File-level import edges only | Cannot answer the useful questions: who calls this, what breaks, why is it here. |
| Learning layer | LLM-written lessons behind quality gates; LLM-written quizzes and "bug hunts"; XP, streaks, confetti | Lessons read as generic prose. Bug-hunt line numbers are not validated. Research finds no evidence that XP and badges help adult professionals. |
| UI | Template landing page, purple chat bubbles, a graph with unreadable nodes and debug counters in the chrome | Generic, and three disconnected pages instead of one workspace. |
| August additions | Neo4j read model (off by default, traversals not reachable from any route or the UI), GraphQL for two queries, a Go gRPC indexer whose README says it is not faster, Kubernetes manifests for one SQLite pod, Terraform for one droplet | Each is carefully built and none is load-bearing. Design rule 8 exists because of them. |
| Deployment | A live public demo, web and API, healthy on 2026-11-15 | It keeps running during the rebuild. |
| Evidence | Few users, no tagged release, and quality claims that mix measured figures with estimates | A rewrite and a rename cost little. The redesign replaces estimates with measurements. |

**Worth carrying forward as ideas, not code:** the "use AI aggressively, trust it nowhere" stance, citation validation against real paths and line counts, deterministic structure with the model confined to narration, the security-audit discipline, and the habit of writing down why.

---

## 3. What the research found

Five research passes covered products and startups, open-source repositories, papers, hosting and pricing, and UI technology.

### 3.1 Saturated: do not build these as the core

| Category | Who owns it |
|---|---|
| Code graph for agents | Graphify (148k stars, Apache-2.0), codegraph (78k, MIT), codebase-memory-mcp (49k), GitNexus (49k, non-commercial licence), code-review-graph (33k), Serena (31k) |
| Graph plus tours for humans | Understand-Anything (93k, MIT): LLM-built graph, dashboard, guided tours, onboarding guide. Also archify (97k), codebase-to-course (HTML course with quizzes) |
| Generated wikis | DeepWiki, Google Code Wiki, Zread, deepwiki-open (18k), openwiki (18k), CodeWiki |
| Repo health and ownership metrics | repowise (8k, AGPL): call graph, blast radius, bus factor, suggested reviewers, decisions mined from history, doc drift. CodeScene (closed) |
| AI PR review | CodeRabbit ($143M round), Qodo ($70M), Greptile ($50M), all 2026 |
| Context engines over MCP | Augment (sold to Harness), Driver, Nia, Unblocked, Tessl |
| Paste-a-URL maps | codeflow (in-browser dependency map), gitdiagram |

### 3.2 Unmet: where the evidence points

1. **Verified understanding instead of explanation.** In Anthropic's randomized trial (n=52), engineers learning a library with AI scored 50% on a comprehension quiz against 67% without. People who delegated scored 24–39%; people who asked conceptual questions scored 65–86%. The gap was largest on debugging. A PNAS study shows the same shape: unguarded AI raised practice scores 48% and cut unassisted exam scores 17%, while a tutor with guardrails avoided the loss.
2. **Authorship no longer implies knowledge.** Every ownership and bus-factor tool above assumes the author understands the code. Two papers (2025, 2026) show agent-written code breaks that assumption, and the second argues the only repair is to measure comprehension directly. A few small CLIs report an agent share or an "AI-adjusted" bus factor (git-pulse, agent-analyzer); none goes on to build understanding or verify it.
3. **Facts separated from narrative.** Maintainers report users filing bugs from hallucinated DeepWiki pages. The best published wiki generator covers 69% of a rubric. A study of LLM-drawn architecture views found a 22.6% clarity-failure rate.
4. **Open-source contributor readiness.** A Linux Foundation survey (Oct 2026) reports maintainers consider 25% of incoming PRs AI slop, and 46% say the need to mentor newcomers has risen. Projects are closing to outside contributions. No tool helps a newcomer learn a repo, show it, then contribute.

### 3.3 Constraints the research imposes

- **A map alone is not a differentiator.** It has to be the canvas for something the graph tools lack. It still must not be a force-directed hairball: the literature favors hierarchical, stable layouts with metric overlays (CodeCity: 24% more correct and 12% faster in a controlled experiment) and finds topic-clustered and whole-graph views confusing.
- **Gamification goes.** XP, streaks and badges show novelty effects that fade in about four weeks, with no evidence for adult professionals.
- **Attempt first.** Productive-failure and retrieval-practice research supports asking for a prediction before giving an explanation. That is also the pattern that preserved learning in the Anthropic trial.
- **Retrieval design is settled enough to copy.** One agent loop with an index tool, grep/read and a symbol graph beats subagent "deep search" (65.2% vs 46.2% on SWE-QA). Off-the-shelf rerankers were net-negative on code.
- **Distribution moved inside the agent.** The 2026 winners ship as skills, plugins and MCP servers that run locally, with a static viewer. Most barely registered on Hacker News. Their recurring complaints: large repos, brittle LLM output, install pain, and files written into the repo without asking.
- **Standalone learning products have a poor record** (CodeSee, Onboard AI, Swimm's pivot). So the first step must be useful with zero effort, and the exercises must live where developers already work.

---

## 4. The product

### 4.1 Thesis

Code used to be understood by whoever wrote it. That link is gone. Fogline makes the gap visible and gives people a way to close it that an LLM cannot do on their behalf.

### 4.2 Measure (deterministic, free, useful in ten seconds)

- **Provenance per line:** human, agent, mixed or unknown. Read from git-ai / Agent Trace records where they exist (exact), else from commit trailers and known bot authors (commit-level and incomplete, so an estimate that can err both ways, labeled as such), else from Fogline's own editor hooks.
- **Evidence of understanding per region:** *strong* (hand-written, substantively hand-edited, or verified by a passed exercise), *weak* (reviewed and approved by a different person), *none* (agent-written and untouched by a person since). Evidence decays with time.
- **Outputs:** the comprehension map; a comprehension-debt figure, raw and risk-weighted; bus factor in classic and evidence-adjusted form; ranked dark hotspots (no evidence × central × frequently changed); reviewers ranked by evidence rather than authorship.

### 4.3 Prove (graded by execution)

| Exercise | Where the answer key comes from |
|---|---|
| Parsons problem on a real function | The real code |
| Predict-then-reveal on a real test | Running the test under a tracer |
| Re-fix a real historical bug | A past fix reverted onto HEAD, valid only if the fix's own tests fail, then pass |
| Seeded bug | A mutation that at least one test kills; the tests decide |
| Explain in plain English | Code is generated from the explanation and run against the function's tests |
| Graph question ("which of these call X?") | The symbol graph |

Inside a coding agent, `/drill` checks a mission out into a worktree and the agent acts as a hints-only tutor. Because Fogline tracks who edits what, a fix the agent wrote does not count as the human's. A pass is recorded as strong evidence and scheduled for review with FSRS.

### 4.4 Learn (LLM-assisted, grounded)

- **Tours:** the order of stops comes from the dependency graph or a recorded trace; the model only narrates each stop. Tours live in the repo as files with symbol anchors and are checked for drift in CI.
- **Guide:** one agent loop with search, read, graph and history tools. Its tool calls also drive the map. Tutor mode asks for a prediction first; "just tell me" is always one click away.
- **Why cards:** the commits, PR descriptions and review threads behind a piece of code, with citations.

### 4.5 The hero screen

A zoomable territory map laid out by directory, stable between commits so spatial memory works. Unknown territory is hatched like the unexplored parts of an old chart. Layers switch between evidence, provenance, risk, churn and ownership. A time slider shows the fog spreading as agents write more. Clicking a region opens the code with a per-line provenance gutter and offers an exercise or a tour.

### 4.6 How it relates to its neighbors

| Neighbor | Relationship |
|---|---|
| Graphify, codegraph, SCIP | **Integrate.** Read their graphs through adapters instead of building another one. |
| git-ai, Agent Trace | **Integrate.** Primary provenance source. |
| Understand-Anything | **Closest on the human side.** No provenance, no evidence model, no verification; tours are unlabeled LLM text and analysis costs tokens. |
| repowise, CodeScene | **Overlap on raw metrics** (ownership, bus factor, hotspots, blast radius). Both treat authorship as knowledge. Do not rebuild their breadth. |
| git-pulse, agent-analyzer | **Overlap on one number.** No line-level history, no map, no loop. |
| DeepWiki, Code Wiki | **Different job.** Fogline's facts are parser-derived and labeled. |
| Claude Code learning style | **Complement.** Per-task, no persistence or map. |
| SWE-bench, SWE-smith | **Borrow the method** (test-validated bugs), point it at people. |

What is new is the combination: line-level provenance, an evidence model that does not equate authorship with knowledge, and execution-graded verification, on one map. Do not claim any single metric is new.

### 4.7 Design rules

1. Facts come from parsers, git and execution. The model narrates, and narrative is labeled.
2. Every statement shown to a user resolves to `file:line@commit`, a graph edge or a test run, or is marked as opinion.
3. Answer keys never come from an LLM.
4. Every number on the site can be reproduced with one command.
5. The index is a static artifact. A server is optional.
6. A developer's own evidence stays on their machine unless they export it. Team views are aggregates.
7. Nothing is written into the user's repository without asking.
8. Every technology in the repo is load-bearing.

### 4.8 Non-goals

Not a productivity or performance-monitoring tool. Not an anti-cheat gate: any quiz can be handed to an AI, so exercises are for the learner and receipts are self-attestation. Not another graph builder, wiki generator or PR reviewer.

---

## 5. Keep, cut, integrate, build

| | Items |
|---|---|
| **Keep (as ideas)** | Trust-nothing validation, deterministic structure, tree-sitter node tables and import-resolution logic (port), CodeTour export, the public-demo rate-limit design |
| **Cut** | RAG chat as the product, Chroma, Neo4j, GraphQL, the Go indexer and gRPC, Kubernetes manifests, the DigitalOcean Terraform, the FastAPI backend, XP and achievements, LLM-written quizzes and bug hunts, the 127 settings and 20 flags |
| **Integrate** | git-ai / Agent Trace · Graphify and codegraph adapters · SCIP indexes for precise references · tree-sitter (grammar pack pinned) · gitoxide · `fsrs-rs` · MapLibre + deck.gl · CodeMirror 6 + Shiki · AI SDK 7 + shadcn AI Elements · MCP SDK + MCP Apps · SWE-bench Lite, Loc-Bench, SWE-QA · cargo-dist (from Phase 3) |
| **Borrow** | Avelino truck-factor algorithm and the degree-of-expertise model · git-of-theseus survival curves · SWE-bench FAIL_TO_PASS validation and SWE-smith bug synthesis · DocAgent's existence ratio · CodeCharta's delta mode · Nx graph's "click an edge to see why" · GitHub Next's README fingerprint |
| **Build (the original part)** | Line-level provenance through history · the evidence model · the comprehension map · the execution-graded exercise engine · verified tours · contributor-readiness paths |
| **Skip** | Own call graph, blast radius, health scores, wiki (adapters cover them) · vector database · LiteLLM (2026 supply-chain incident) · Serena app and repowise as linked dependencies (GPL, AGPL) · Kuzu, stack-graphs, hercules (dead) · Remotion, deepscatter, Cosmograph (licences) · sync engines · off-the-shelf rerankers |

---

## 6. Architecture

```
                 your repo (laptop or CI)
                          |
              +-----------v------------+
              |  engine (Rust binary)  |  scan, history, provenance,
              |  SQLite store          |  evidence model, exercises
              +--+-------+-------+-----+
                 |       |       |
        static bundle  local   MCP server ---- coding agents
        (tiles, shards) server            <--- plugin hooks: who edited what,
                 |     (HTTP, WS)               exercises passed
              +--v-------v--+
              |   viewer    |  map, code, guide   (one React package:
              +------+------+   site, local, MCP app)
                     |
        browser calls the LLM directly with the user's key,
        or a capped free quota through a small edge Worker

Hosted: static site and showcase bundles on Cloudflare (Workers static assets + R2),
        bundles rebuilt by GitHub Actions in this repo.
```

- **Engine.** One Rust binary: `scan`, `debt`, `who`, `map`, `drill`, `tour`, `serve`, `mcp`. SQLite is the store, with a documented schema anyone can query.
- **Bundle.** Exported from SQLite as map tiles plus small columnar shards. The hosted viewer needs no database and paints a skeleton within a second.
- **Viewer.** A framework-agnostic React package with two data sources behind one interface: static bundle or local server.
- **Agent loop.** Runs in the browser; tools execute against the local bundle, so with the user's own key the code never touches a Fogline server.
- **Execution.** Exercises run tests on the user's machine or CI, in a container by default.
- **Privacy.** No file contents leave the machine unless the user publishes a bundle. Personal evidence lives in `~/.fogline`.

---

## 7. Stack: one choice per layer

| Layer | Choice | Why | Fallback |
|---|---|---|---|
| Engine | Rust (git plumbing first, gitoxide where it measures faster; tree-sitter, rusqlite, axum, rmcp) | One self-contained binary through npm, Homebrew and cargo; fast startup for editor hooks; headroom for line-level history | TypeScript on Node, decided at the Phase 1 gate |
| Store | SQLite, with FTS5 for search | Zero-config, portable, queryable by users | DuckDB for analytics only |
| Map | MapLibre GL JS 6 with a deck.gl 9.4 overlay, on WebGL2 | Label decluttering, zoom-banded styling and fly-to for free; WebGPU is not yet universal | deck.gl `OrthographicView` alone, decided by a two-day spike |
| Viewer | React 19, CodeMirror 6, Shiki 4, Motion, cmdk, nuqs | Read-mostly code at scale; every view is a URL | — |
| Design system | shadcn/ui on Base UI, Tailwind 4.3, tokens in OKLCH, a typeface other than Inter | Avoids the template look the current site has | — |
| Site | Next.js 16 static export on Cloudflare | Free, unlimited static requests survive a traffic spike; Vercel Hobby can pause a project for up to 30 days on overage | TanStack Start |
| Guide | AI SDK 7 with client-side tools | Tool calls double as map controls | assistant-ui |
| LLM | User's own key (Anthropic, OpenAI, Google) or OpenRouter sign-in; free quota on Workers AI or Haiku 5.5 behind a spend cap | Haiku 5.5 is $0.10 / $0.50 per million tokens | — |
| Evals and study | Python with uv, LightGBM, lifelines | The ecosystem for benchmarks and survival analysis | — |
| Agent surface | Claude Code plugin (skills, hooks, MCP), then other agents through MCP | Where adoption happens | — |
| CI/CD | GitHub Actions on native runners for all five targets, cargo-deny, Playwright, build-provenance attestations; cargo-dist reconsidered in Phase 3 | Cross-platform binaries, verifiable supply chain | — |
| Infra as code | OpenTofu for the Cloudflare bucket, Worker, DNS and Turnstile | Small and load-bearing | — |

Rust for the engine, TypeScript for everything a person sees, Python for measurement. No Go, GraphQL or Kubernetes; nothing here needs them.

---

## 8. The ten phases

Effort is in focused weeks for one person working with an AI pair. The building may compress. The validation work (oracles, benchmarks, studies) does not, and it is what makes the project credible.

| # | Phase | Weeks | Release |
|---|---|---|---|
| 1 | Reset and tracer bullet | 2 | |
| 2 | Engine: history, provenance, structure | 4 | |
| 3 | Evidence model, CLI report, first study | 3 | **Alpha 1:** CLI, agent skill, study |
| 4 | The map | 4–5 | **Alpha 2:** public gallery |
| 5 | Prove: exercises in the agent | 4 | **Beta:** the loop is closed |
| 6 | Agent-native depth: hooks, live mode, MCP app, PR action | 3 | |
| 7 | Retrieval and evals | 4 | |
| 8 | The guide: ask, tours, why | 4 | |
| 9 | Contributor readiness, teams, more exercise types | 4 | |
| 10 | Hardening, pilot study, launch | 3–4 | **1.0** |

Stop-and-check at each release. If Alpha 2 draws no outside users within a month, narrow to whichever surface people used instead of pushing through all ten. Package versions start at 0.1.0, and the launch release is 1.0.0.

### Phase 1 — Reset and tracer bullet

**Goal.** Close v1 cleanly and run the whole pipeline in its thinnest form.

**Build.**
- Leave v1 intact in its own repository: tag it and keep its live demo. Build Fogline in a new public repository. Full detail in the [Phase 1 spec](specs/phase-1.md).
- New layout: `crates/`, `packages/viewer`, `docs/adr/`; `apps/site`, `plugins/`, `action/` and `evals/` are added by the phase that first needs them.
- Engine v0: walk the repo, count lines, read the git log for churn and agent trailers, write a bundle.
- Viewer v0: treemap colored by agent-attributed share.
- `npx fogline` scans and opens the map.
- CI and release pipeline for macOS, Linux and Windows.
- Two-day renderer spike on a synthetic 50k-file repo, recorded as an ADR.

**Hard part.** Shipping a Rust binary through npm on three platforms.

**Done when.** `npx` to a rendered map in under 15 s on a 5k-file repo (*target*), green on all three platforms, pre-release tagged.

**Gate.** If Rust costs more than it returns after two weeks, switch the engine to TypeScript and continue.

### Phase 2 — Engine: history, provenance, structure

**Goal.** The deterministic facts everything else stands on.

**Build.**
- Line-level history: first with parallel blame (correct by construction), then an incremental tracker that replays history once, validated against the first.
- Provenance resolution in priority order: git-ai / Agent Trace, trailers and bot authors, unknown.
- History metrics as inputs to risk: churn, ownership, co-change, file age, optional review evidence from the GitHub API.
- Structure: symbols and file-level imports with tree-sitter for TypeScript/JavaScript, Python, Go, Rust and Java.
- Adapters for richer graphs: SCIP, Graphify `graph.json`, codegraph. Every edge records where it came from.
- Incremental re-scan and watch mode.
- A benchmark corpus of ten real repos, small to very large, tracked in CI.

**Hard parts.** Blame semantics across merges and renames; identity resolution; history at 100k+ commits.

**Done when** (*targets*). The tracker agrees with blame on at least 99% of lines across 1,000 sampled files. Import-edge precision is at least 95% against SCIP on three repos, recall reported. Re-scanning one new commit on a 500k-line repo takes under 500 ms.

### Phase 3 — Evidence model, CLI report, first study

**Goal.** The central idea in a form people can run in ten seconds.

**Build.**
- The evidence model (§4.2) with documented default weights and decay.
- Comprehension debt, classic and evidence-adjusted truck factor (a port of Avelino et al.), dark hotspots, reviewers ranked by evidence.
- `fogline debt`, `fogline who <path>`, `fogline dark`, with terminal, JSON and Markdown output.
- A model card: assumptions, limits, sensitivity analysis, ethics position.
- A thin Claude Code skill wrapping the CLI.
- **Study 1:** run the engine over 100–200 active open-source repos. Report agent-attributed share over time (signatures undercount, and the report says so), line survival by provenance (Kaplan–Meier and a Cox model), and classic versus adjusted truck factor. Publish the dataset and notebook.

**Hard part.** A model of evidence that is honest about being a model.

**Done when.** Classic truck factor reproduces published values on repos from the original paper. Rankings are stable under weight changes (rank correlation reported). The study reruns with one command. **Alpha 1** ships with the study as its launch post.

### Phase 4 — The map

**Goal.** The screen that makes someone stop scrolling.

**Build.**
- A visual identity first, in two or three explorations, before any component work.
- Stable territory layout by directory, computed in the engine.
- Semantic zoom: districts, files, symbols, then real code in a side panel with a per-line provenance gutter.
- Layers on number keys: evidence, provenance, risk, churn, ownership.
- Dependency edges bundled along the directory tree, shown only for the hovered or selected node.
- Command palette as map search; every view is a URL.
- Time slider; delta mode for a PR or release.
- Gallery site with ten or more showcase repos as static bundles; a repo fingerprint for social cards and a README badge.

**Hard parts.** Label legibility at every zoom; camera state outside React; first paint.

**Done when** (*targets*). 60 fps at 50k files on an M-series laptop, 30 fps on integrated graphics. Skeleton under 1 s, interactive under 2.5 s. Keyboard navigation, contrast and reduced motion pass. Visual regression tests in CI. Hosting cost $0. **Alpha 2** launches.

### Phase 5 — Prove: exercises in the agent

**Goal.** The part no neighbor has. Close the loop.

**Build.**
- Test discovery and selective runs for pytest and vitest/jest; a coverage map from tests to code; container sandbox by default.
- Three exercise types: Parsons problems, predict-then-reveal, and re-fixing real historical bugs (FAIL_TO_PASS validated).
- `/drill` in Claude Code: mission in a worktree, hints-only tutor, human edits distinguished from agent edits.
- FSRS scheduling; passes flow into the evidence model and lift the fog on the map.
- Two or three curated missions that run in the browser for the public demo.

**Hard parts.** Running other people's test suites reliably; picking bugs that teach something.

**Done when** (*targets*). At least 95% of generated missions pass automatic validation. A full measure–drill–map round trip works on five showcase repos. **Beta** ships.

### Phase 6 — Agent-native depth

**Build.**
- Hooks that log agent-edited line ranges when git-ai is absent.
- A session summary on stop ("412 agent-written lines across 9 files, 3 in dark hotspots") with an opt-in checkpoint.
- MCP server with lean, facts-only tools, and an MCP app that renders the map inside the chat host.
- Live mode: the map pulses where the agent is editing; sessions can be replayed.
- Codex, Cursor and Gemini CLI through MCP.
- GitHub Action: on a PR, comment with lines by provenance, dark territory touched and reviewers ranked by evidence; on main, publish the bundle.

**Done when** (*targets*). One-command install. Hook overhead under 50 ms at p95. The Action finishes in under two minutes on a 500k-line repo with cache. Three repos that are not yours use it.

### Phase 7 — Retrieval and evals

**Goal.** Measured retrieval, replacing 48 hand-tuned constants.

**Build.**
- Lexical search on FTS5, an optional small CPU embedding model, deterministic context headers per chunk, graph navigation through the adapters.
- A harness for issue-to-code localization on SWE-bench Lite (274 instances) and Loc-Bench (560).
- A learned ranker (LightGBM, LambdaMART) over lexical, dense, path, graph, churn and co-change features, trained on issue-to-fix pairs mined from repos disjoint from the eval sets.
- SWE-QA for the question-answering loop, with cost per correct answer.
- A benchmarks page generated from CI artifacts.

**Done when.** Numbers are published with reproduction commands and an ablation table. If the learned ranker does not beat simple fusion, the page says so and the simpler one ships.

### Phase 8 — The guide: ask, tours, why

**Build.**
- The single-loop agent with search, read, graph, history and map-control tools; tutor mode.
- Fact and narrative labels on every statement; a stale badge when cited code has changed.
- Tour skeletons from dependency or trace order; narration per stop; tours as files with symbol anchors; a drift check for CI; export to CodeTour and Markdown.
- Why cards from commits, PRs and reviews, cached by content hash.
- User's own key, OpenRouter sign-in, and the capped free quota.

**Done when** (*targets*). At least 98% of entities cited in generated tours resolve. The drift check catches every seeded anchor break. A generated tour costs under $0.05.

### Phase 9 — Contributor readiness, teams, more exercise types

**Goal.** The open wedge: help a newcomer learn a repo, show it, then contribute.

**Build.**
- Maintainer kit: `fogline init --oss` proposes curated tours, a readiness path and a contribution map.
- Issue to "read these first", using the Phase 7 ranker.
- A first-timer Action: a welcome comment with the path, and a context card for reviewers.
- A readiness receipt, clearly labeled as self-attestation.
- Team onboarding: `fogline plan --for <area>` prioritizes unknown regions for a new hire; an aggregate view shows bus-factor risks.
- More exercises: seeded bugs, explain-in-plain-English, graph questions; Go and Rust test runners.

**Done when.** Three real open-source projects adopt the kit, and one case study is written.

### Phase 10 — Hardening, pilot study, launch

**Build.**
- Threat model: untrusted repos, prompt injection through repo text (tools are read-only and cannot reach the network), sandbox boundaries, supply chain. Parser fuzzing.
- Performance budgets enforced in CI; accessibility audit.
- Pilot study with 8–12 developers: comprehension after Fogline against chat alone, reporting effect size and interval, and stating what the sample can and cannot show.
- Docs site: architecture, model card, benchmark methodology.
- Launch the way the 2026 winners did: a one-line pitch ("any repo → a map of the code no human has read"), one command, a gallery of famous repos, a 60-second video, the README in several languages, and plugin marketplaces ahead of Hacker News.
- 1.0 release.

**After 1.0, only on demand:** VS Code extension, in-browser scanning through WASM, a desktop build, a team server.

---

## 9. Numbers this project will publish

| Metric | Oracle or benchmark | Published reference | First target |
|---|---|---|---|
| Line-tracking accuracy | `git blame` | — | ≥ 99% agreement |
| Import-edge precision | SCIP | — | ≥ 95%, recall reported |
| Mission validity | The repo's test suite | — | ≥ 95% |
| Map performance | Fixed scenes | — | 60 fps at 50k files; skeleton under 1 s |
| File localization, SWE-bench Lite (Acc@5) | Gold patches | BM25 61.7 · CodeRankEmbed 84.7 · LocAgent 94.2 | ≥ 85 with no LLM; ≥ 90 with one cheap-model loop under $0.05 per instance |
| File localization, Loc-Bench (Acc@5) | Gold patches | CodeRankEmbed 74.3 · LocAgent 83.4 | Beat the embedding baseline |
| Repo QA, SWE-QA | The paper's judge protocol | RAG 66.6 · agents about 70.8 | ≥ 68 at under 10k input tokens per question |
| Existence ratio of generated tours | Static graph | DocAgent 95.7% vs 61.1% for plain chat | ≥ 98% |
| Agent share and line survival | 100+ repos | — | Dataset and notebook public |
| Comprehension effect | Pilot study | Anthropic RCT: 50% vs 67% | Effect size and interval reported |

---

## 10. Cost

| Item | Monthly |
|---|---|
| Site and viewer on Cloudflare Workers static assets | $0 |
| Showcase bundles on R2 (10 GB free, no egress fees) | $0 |
| Showcase indexing on GitHub Actions (public repo) | $0 |
| Domain | about $1 |
| Free LLM quota: Workers AI daily allowance (stops at zero cost), or Haiku 5.5 behind a workspace spend limit | $0–5 |
| Error tracking and analytics on free tiers | $0 |
| **Total** | **$1–6** |

One-time LLM spend, all optional and cached: showcase tours and why cards about $1–5 (5,000 summaries cost roughly $0.81 on Haiku 5.5 batch); SWE-QA runs $10–40; the localization agent loop about $40.

If dynamic quotas ever bind, Workers Paid is $5 per month. Nothing in the design needs a VPS, a managed database or a vector store.

---

## 11. Risks, and what would change the plan

| Risk | Response |
|---|---|
| Scope is large | Three releases with stop-and-check points; Phases 1–5 are a complete, differentiated project |
| The space moves monthly; neighbors with 100k stars could add assessment | Integrate with them rather than compete on graphs; execution-graded verification and line-level provenance are slow to copy |
| Raw metrics overlap with repowise and small CLIs | Do not claim the metric is new; lead with the loop, the evidence model and the validation |
| Signature-based provenance is commit-level and incomplete; many agents leave no trace | Say so in the UI; exact data comes from Agent Trace and hooks as adoption grows |
| The evidence model reads as surveillance or pseudo-science | Model card, sensitivity analysis, local personal data, aggregate team views, stated non-goals |
| Running arbitrary test suites is messy | Python and TypeScript first; container by default; showcase missions pre-validated in CI |
| Rust slows the build | Phase 1 gate with a TypeScript fallback |
| The MapLibre coordinate approach fails | Two-day spike; deck.gl alone is the fallback |
| Learning tools have weak pull | Measure is useful with zero effort; exercises live inside the agent; launch early and follow usage |
| Name or trademark conflict | Check before Phase 1 ends |

---

## 12. Open decisions

1. **Engine language.** Rust, subject to the gate at the end of Phase 1 ([spec](specs/phase-1.md), section 10). The fallback is TypeScript on Node.
2. **Name.** "Fogline" is free on npm, crates.io and PyPI. The fog line is the painted edge line that keeps drivers oriented in fog. The name becomes final before the first publish.
3. **v1.** Settled: v1 stays untouched in its own repository, so nothing has to be removed ([ADR-0001](adr/0001-repository-name-versions-conventions.md)).

---

## 13. Sources

**Landscape**
- [Graphify](https://github.com/Graphify-Labs/graphify) · [Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) · [codegraph](https://github.com/colbymchenry/codegraph) · [GitNexus](https://github.com/abhigyanpatwari/GitNexus) · [code-review-graph](https://github.com/tirth8205/code-review-graph) · [repowise](https://github.com/repowise-dev/repowise) · [codeflow](https://github.com/braedonsaunders/codeflow) · [codebase-to-course](https://github.com/zarazhangrui/codebase-to-course) · [deepwiki-open](https://github.com/AsyncFuncAI/deepwiki-open)
- [DeepWiki docs](https://docs.devin.ai/work-with-devin/deepwiki) · [Google Code Wiki](https://developers.googleblog.com/introducing-code-wiki-accelerating-your-code-understanding/) · [Cognition Codemaps](https://cognition.com/blog/codemaps)
- [OpenWrt maintainers on DeepWiki inaccuracies](https://forum.openwrt.org/t/important-consideration-deepwiki-ai-generated-docs-may-contain-inaccuracies/249471) · [DeepWiki critique](https://blopker.pages.dev/writing/12-deepwiki) · [AI wikis as docs theater](https://passo.uno/ai-wikis-docs-teather-as-a-service/)
- [Linux Foundation survey on AI slop PRs](https://www.linuxfoundation.org/press/new-lf-research-survey-shows-generative-ai-cuts-open-source-coding-time-in-half-but-25-of-incoming-pull-requests-are-considered-ai-slop) · [InfoQ: projects closing to AI contributions](https://www.infoq.com/news/2026/02/ai-floods-close-projects/) · [Sonar developer survey](https://www.sonarsource.com/blog/state-of-code-developer-survey-report-the-current-reality-of-ai-coding/)
- Funding: [CodeRabbit Series C](https://www.coderabbit.ai/newsroom/coderabbit-series-c-agentic-change-management) · [Greptile Series B](https://www.greptile.com/blog/series-b) · [Augment and Harness](https://augmentcode.com/blog/augment-code-next-chapter)

**Comprehension and knowledge models**
- [Anthropic: AI assistance and coding skills (RCT)](https://www.anthropic.com/research/AI-assistance-coding-skills) · [PNAS 2025: generative AI can harm learning](https://doi.org/10.1073/pnas.2422633122) · [METR 2026 update](https://metr.org/blog/2026-02-24-uplift-update/)
- [Comprehension debt (Gorman)](https://codemanship.wordpress.com/2025/09/30/comprehension-debt-the-ticking-time-bomb-of-llm-generated-code/) · [Addy Osmani](https://addyosmani.com/blog/comprehension-debt/) · [Cognitive debt (Storey)](https://margaretstorey.com/blog/2026/02/18/cognitive-debt-revisited/)
- [The Substrate Collapse: AI code generation invalidates authorship-based knowledge metrics](https://arxiv.org/pdf/2606.20882) · [Impact of generative AI on code expertise models (Cury and Avelino)](https://arxiv.org/abs/2507.08160) · [Truck factor algorithm](https://doi.org/10.1109/ICPC.2016.7503718) · [Truck-Factor reference implementation](https://github.com/aserg-ufmg/Truck-Factor)
- [Agent Trace spec](https://agent-trace.dev/) · [cursor/agent-trace](https://github.com/cursor/agent-trace) · [git-ai](https://github.com/git-ai-project/git-ai) · [Thoughtworks Radar on Agent Trace](https://www.thoughtworks.com/radar/platforms/agent-trace)
- Related small tools: [git-pulse](https://pypi.org/project/gitpulse-ai/) · [agent-analyzer](https://awesome.ecosyste.ms/projects/github.com%2Fagent-sh%2Fagent-analyzer) · [git-tribe](https://pypi.org/project/git-tribe/)

**Retrieval, benchmarks, grounding**
- [SWE-QA](https://arxiv.org/abs/2509.14635) · [LocAgent and Loc-Bench](https://arxiv.org/abs/2503.09089) · [Deep agentic vs semantic search](https://arxiv.org/abs/2608.01507) · [Cursor on semantic search](https://cursor.com/blog/semsearch) · [Chroma: context rot](https://www.trychroma.com/research/context-rot)
- [CodeRankEmbed](https://huggingface.co/nomic-ai/CodeRankEmbed) · [voyage-code-4](https://blog.voyageai.com/2026/08/13/voyage-code-4/) · [Anthropic contextual retrieval](https://www.anthropic.com/news/contextual-retrieval)
- [DocAgent](https://arxiv.org/abs/2504.08725) · [CodeWiki and CodeWikiBench](https://arxiv.org/abs/2510.24428) · [Architecture-view generation study](https://arxiv.org/abs/2603.21178) · [SWE-Flux](https://arxiv.org/abs/2609.28449)

**Learning science and visualization**
- [FSRS benchmark](https://github.com/open-spaced-repetition/srs-benchmark) · [Explain-in-plain-English autograding](https://doi.org/10.1145/3649217.3653582) · [Parsons problems](https://doi.org/10.1145/3411764.3445292) · [Productive failure meta-analysis](https://doi.org/10.3102/00346543211019105) · [Gamification meta-analysis](https://doi.org/10.1007/s10648-019-09498-w) · [Gamification novelty effect](https://doi.org/10.1186/s41239-021-00314-6)
- [Good first issues study](https://doi.org/10.1145/3368089.3409746) · [Newcomer barriers in OSS](https://doi.org/10.1145/2675133.2675215)
- [CodeCity controlled experiment](https://wettel.github.io/codecity-experiment.html) · [Trace visualization experiment](https://doi.org/10.1109/TSE.2010.47) · [CodeVoyager](https://doi.org/10.1145/3742413.3789057)

**UI and platform**
- [Next.js 16.4](https://nextjs.org/blog/next-16-4) · [TanStack Start 1.0](https://tanstack.com/blog/tanstack-start-1-0) · [Railway leaving Next.js](https://blog.railway.com/p/moving-railways-frontend-off-nextjs)
- [deck.gl](https://deck.gl/docs/whats-new) · [MapLibre GL JS 6](https://maplibre.org/maplibre-gl-js/docs/guides/v5-to-v6-migration-guide/) · [WebGPU support](https://caniuse.com/webgpu) · [AI SDK 7](https://vercel.com/blog/ai-sdk-7) · [shadcn AI Elements](https://ui.shadcn.com/docs/changelog/2026-10-ai-elements)
- [Linear's redesign notes](https://linear.app/now/how-we-redesigned-the-linear-ui) · [Vercel interface guidelines](https://vercel.com/design/guidelines)

**Hosting and pricing**
- [Cloudflare Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/) · [R2 pricing](https://developers.cloudflare.com/r2/pricing/) · [Workers AI pricing](https://developers.cloudflare.com/workers-ai/platform/pricing/) · [Vercel Hobby limits](https://vercel.com/docs/plans/hobby)
- [Claude pricing](https://platform.claude.com/docs/en/about-claude/pricing) · [GitHub Actions terms](https://docs.github.com/en/site-policy/github-terms/github-terms-for-additional-products-and-features) · [OpenRouter OAuth](https://openrouter.ai/docs/guides/overview/auth/oauth) · [LiteLLM malicious release advisory](https://osv.dev/vulnerability/MAL-2026-2144)
