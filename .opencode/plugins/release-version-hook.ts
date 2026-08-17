// release-version-hook.ts — auto-fires the release-version-check skill.
//
// The moment the user talks about releasing / bumping the version ("релизни
// версию", "bump", "новая версия", "выпусти", ...), this hook appends a
// short reminder to the user message so the agent runs the
// release-version-check skill BEFORE touching version.json / APP_VERSION /
// tags. The skill itself (in .opencode/skills/) does the actual verification
// — this is only the trigger.
//
// Design notes:
// - Deterministic wiring: AGENTS.md already instructs the model, but a
//   message-level reminder makes the skill fire even if the release request
//   comes in a short, ambiguous phrase.
// - False positives (the word "release" in an unrelated context) only cost a
//   reminder line, so the regex is deliberately broad.
// - The hook never edits config or runs commands — appending to the message
//   is the only side effect, wrapped in try/catch so a plugin API change
//   degrades to "no reminder" instead of breaking message handling.
function releaseIntent(text) {
  const releaseVerbs = /(релиз|выпуст|выложи версию|сделай версию|bump|release|подними версию|новую версию|новая версия)/i
  const anyVersion = /v?\d+\.\d+(\.\d+)?/i
  return releaseVerbs.test(text) || (anyVersion.test(text) && /(релиз|версия|версию|тег|tag)/i.test(text))
}

const REMINDER = [
  "⛔ [release-version-check] The user is asking about a new release/version.",
  "Load the release-version-check skill and run it BEFORE choosing/bumping the",
  "version (check version.json vs APP_VERSION sync, remote tags, latest release,",
  "CHANGELOG placeholders).",
].join(" ")

export const ReleaseVersionHook = async () => {
  return {
    "chat.message": async (input) => {
      try {
        const text = String(input?.message?.content ?? "")
        if (!releaseIntent(text)) return
        const fresh = [text, REMINDER].join("\n\n")
        // content may be a plain string or a parts array; prefer appending
        // to the string form and leave structured content untouched.
        if (typeof input.message.content === "string") {
          input.message.content = fresh
        } else if (Array.isArray(input.message.content)) {
          input.message.content.push({ type: "text", text: REMINDER })
        }
      } catch {
        // never break message handling because of the reminder
      }
    },
  }
}