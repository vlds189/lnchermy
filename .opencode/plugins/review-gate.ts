// review-gate.ts — self-review gate before release builds.
//
// Blocks `cargo build --release` while the working tree has uncommitted
// changes whose diff hash does not match the last reviewed one. The first
// attempt is denied with a message telling the agent to run the code-review
// skill on the diff; a retry after the review records the diff hash in
// `.opencode/review-gate.sha` and is allowed. A NEW diff (more edits after a
// review) is blocked again.
//
// Design notes:
// - The gate cannot detect "a review happened" on its own, so the retry IS
//   the signal — this is a speed bump that forces at least one review pass
//   per diff, not a watermark that proves nothing.
// - The hash covers `git diff` output (tracked changes) PLUS
//   `git status --porcelain` (the file list). Status alone would let edits
//   to already-modified files slip through; diff alone would miss untracked
//   files. Together they detect any change to the handover artifact.
// - `cargo check`, `cargo test` and non-release builds are never touched —
//   only the handover artifact (release exe) is gated.
import { readFileSync, writeFileSync } from "node:fs"
import { join } from "node:path"

const GATE_MESSAGE = [
  "⛔ Release build blocked: working tree has uncommitted changes that have not",
  "passed code review yet.",
  "1. Load the code-review skill and run it over `git diff` (plus new files).",
  "2. Fix the findings it reports.",
  "3. Re-run this same build command — the retry confirms the review is done.",
].join(" ")

// Matches `cargo build ... --release` (or `-r`), ignoring pipes/`;` in the
// command so PowerShell chaining like `... | Select-Object` still matches.
const RELEASE_BUILD = /cargo\s+build\b[^\n|;]*(--release|-r)\b/

export const ReviewGate = async ({ worktree, directory }) => {
  const root = worktree || directory || "."
  const shaPath = join(root, ".opencode", "review-gate.sha")
  let lastBlockedHash = ""

  const gitOut = (args: string[]) => {
    const res = Bun.spawnSync(["git", "-C", root, ...args])
    return res.exitCode === 0 ? res.stdout.toString() : null
  }

  const worktreeHash = () => {
    const diff = gitOut(["diff"])
    const status = gitOut(["status", "--porcelain"])
    if (diff === null || status === null) return null
    return diff + "\n" + status
  }

  const diffHash = (state) => {
    if (!state) return null
    return new Bun.CryptoHasher("sha256").update(state).digest("hex")
  }

  const storedHash = () => {
    try {
      return readFileSync(shaPath, "utf8").trim()
    } catch {
      return null
    }
  }

  return {
    "tool.execute.before": async (input, output) => {
      if (input.tool !== "bash") return
      const cmd = String(output?.args?.command ?? "")
      if (!RELEASE_BUILD.test(cmd)) return

      const hash = diffHash(worktreeHash())
      if (!hash) return // not a git repo or git unavailable — no gate

      // Diff already reviewed (recorded on a previous retry) → allow.
      if (storedHash() === hash) return

      if (lastBlockedHash === hash) {
        // Retry after the blocked attempt: the agent confirmed the review.
        writeFileSync(shaPath, hash, "utf8")
        return
      }

      lastBlockedHash = hash
      throw new Error(GATE_MESSAGE)
    },
  }
}