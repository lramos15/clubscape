# Initial task-ledger workflow

`milestones/m1-tasks.json` records task scope, dependencies, path ownership,
source/parity references and validation. Only ready tasks with clear relevant
contracts may implement. Read-only research/review needs no worktree; writers
use isolated branches/worktrees and serialize shared contracts and lockfiles.

The Director maintains one shared agent budget. Reservations count against the
25-agent ceiling before spawn. Every nested task, retry and resumption would
require fresh admission; initial workers are explicitly forbidden from
delegating. Completion or confirmed parking/cancellation is recorded before
release. Never create replacement workers to bypass throttling.

At this checkpoint, the Director is the sole coordinator. No independent
coordinator or other active worker is visible. Do not start another execution
coordinator against the same ledger concurrently; distributed admission is not
implemented or claimed. Pause admissions if another coordinator is discovered
until ownership can be reconciled.

On rate limits, honor provider retry instructions, stop new admissions and
reduce effective concurrency. Do not repeatedly respawn failing tasks. Exact
provider accounting may be unavailable and must be labeled as such.

This minimal workflow is not the WaddleWorks MVP. Only the first milestone is
authorized; no automatic scheduling loop may advance its successors.
