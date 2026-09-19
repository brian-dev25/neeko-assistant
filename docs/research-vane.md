# Research pipeline adapted from Vane

Reference reviewed: https://github.com/ItzCrazyKns/Vane/blob/master/src/lib/agents/search/researcher/actions/search/baseSearch.ts and src/lib/agents/search/index.ts (2026-09-12).

## Architecture (2026-09-14 update)

Neeko now uses a tool-based research agent instead of the previous two-round fixed loop. The agent plans, searches, reads pages, follows links, and writes answers with citations in a budget-controlled cycle.

### Components

| Module | Role |
|---|---|
| `research/types.rs` | Data contracts: `ResearchState`, `Document`, `Fragment`, `LinkId`, `Budget`, `ModelDecision` |
| `research/agent.rs` | Main loop: resolve question → tool loop → write answer |
| `research/tools.rs` | Tool execution: search, read, follow_link, github_read, reddit_read, finish |
| `research/prompts.rs` | Model prompts: planner, writer, verifier, resolver |
| `research/verification.rs` | Citation validation, URL validation, source ID remapping |
| `research/progress.rs` | Progress service: backend state, Tauri command, HTTP route |
| `research/reader.rs` | Enhanced HTML reader with fragments, links, incremental download |
| `research/providers/github.rs` | GitHub API: repo, README, releases, issues |
| `research/providers/reddit.rs` | Reddit API: posts, comments |
| `research/google.rs` | Google search via headless Edge (unchanged) |
| `research/cache.rs` | In-memory TTL cache (unchanged) |

### Agent flow

1. **Resolve**: Model extracts `resolved_question`, `intent`, and `entities` from the user's question.
2. **Plan**: Model receives operational summary and chooses a tool operation.
3. **Execute**: Backend validates and runs the operation (search/read/follow/github_read/reddit_read).
4. **Repeat**: Steps 2-3 continue until the model chooses `finish` or budget runs out.
5. **Write**: Model generates the final answer with evidence citations.
6. **Verify**: Citation and fragment IDs and URLs are checked, then a model pass reviews factual support. Validation failure returns an inability-to-verify message; references are not silently removed to publish an unsupported answer.

Before this flow, `assistant.rs` classifies factual lookup separately from the Neeko character prompt when research is enabled. This covers misspelled entity questions without a product-name allowlist. Recent conversation context is passed to the resolver. There is no production fallback to the old two-round loop.

Audit note: the previous completion report overstated coverage. End-to-end model evaluation remains necessary; see the posterior audit in `research-agent-plan.md`. Some declared limits/strategies still require complete enforcement (notably follow depth, authority verification, context token budgeting, and progress session isolation). Passing unit tests is not evidence that all planned behavior works in the installed app.

### Budget defaults

| Resource | Limit |
|---|---|
| Total operations | 8 |
| Searches | 4 |
| Documents read | 6 |
| Follow depth | 3 |
| Candidate sources | 24 max, 6 new per search |
| Model decisions | 8 + writer + verifier |
| Research deadline | 180s total, 45s reserved for writing |
| HTTP timeout | 12s per request |
| Document size | 1MB download, 6000 chars extracted |
| Writer token budget | 800-1600 tokens |

### Differences from upstream Vane

- Uses existing Google/website API providers instead of requiring SearxNG
- Tool-based architecture instead of fixed rounds
- Budget enforcement at every step
- Stable source IDs (never reassigned after discovery)
- Citation validation against actual evidence
- Progress reporting to all three interfaces (desktop, chat window, web)
- Does not implement Vane's 25-iteration quality mode

### Progress reporting

The backend stores the latest progress state per request/generation. Clients poll every 800ms via:
- Tauri: `research_progress` command
- Web: `GET /api/research/progress?request_id=...`

Phases: `planning → searching → reading → checking → writing → complete`

HTML extraction excludes navigation and duplicate nested blocks, and chooses paragraphs within a character budget. Citation IDs and selected result indices are checked against actual retrieved sources. This checks reference integrity, not factual entailment: model relevance and grounding still require live evaluation.
