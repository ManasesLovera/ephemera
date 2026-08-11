# Release notes guide

Two documents get written for every release, aimed at two different readers.

| Where | Audience | Tone |
| --- | --- | --- |
| GitHub release notes (`gh release edit`/`create`) | Anyone downloading the app | High level, no jargon |
| [`releases/vX.Y.Z.md`](releases/) | Contributors, future-you debugging a regression | Technical, detailed |

**Never put technical detail in the GitHub release notes.** A user picking a binary to
download doesn't need to know which review model caught a bug or which Slint API
lacks file-path support — that goes in `releases/`, linked from the GitHub notes if
it's worth pointing at.

## GitHub release notes — required sections

Keep it short. A user should be able to read the whole thing in under a minute and
know: what changed, whether it affects them, and how to get it.

```markdown
<One or two sentences: what this release is, in plain language.>

## Download

See [DOWNLOAD.md](https://github.com/ManasesLovera/ephemera/blob/main/DOWNLOAD.md)
for which file to pick and what each tier needs at runtime.

## Improvements

- <New feature or enhancement, described by what the user can now do — not by the
  file/function that changed.>

## Bug fixes

- <What was broken, described by the symptom a user would have seen — not the root
  cause or the fix mechanism.>
```

Omit a section entirely if it's empty rather than writing "N/A" or "none this release."

**Write for the download-and-run reader, not the reader of this repo.** "Fixed the
Refresh button being unreadable" — not "swapped the generic `std-widgets::Button` for
`PaneButton`." If a change has no user-visible effect (a CI config tweak, an internal
refactor, a doc update), it usually doesn't belong in the GitHub notes at all — it
belongs in `releases/vX.Y.Z.md` only.

## `releases/vX.Y.Z.md` — required sections

This is the technical record. Write it like an engineering changelog: specific enough
that reading it later answers "why did we do this" and "what exactly changed" without
having to `git log` your way through every commit since the last tag.

```markdown
# vX.Y.Z

<Short technical summary — what this release actually is, one level more detail than
the GitHub notes.>

## Changes

<Grouped by area (UI, core, CI, docs, etc.) if there's enough to group. Each entry
names the actual files/components/APIs touched, not just the user-facing effect.>

## Documentation

<Which docs/*.md files changed and why, if any. Skip this section if nothing in
docs/ changed.>

## CI / pipeline

<What changed in .github/workflows/, build tooling, or release process itself, if
anything. Skip if nothing changed here.>

## Known gaps

<Anything left open, deferred, or deliberately not done — and why. Carry forward
gaps from the previous release's file that are still open; note ones that got
closed.>

## Technical details

<Root causes, API constraints hit, measurements taken, anything a future debugging
session would want. This is the place for "here's exactly what broke and how it was
diagnosed.">
```

Skip a section if it's genuinely empty for this release — don't pad it.

## Workflow

Both documents get written together, as part of creating a release (see the release
procedure in [`CLAUDE.md`](CLAUDE.md)):

1. Build the full list of changes since the last tag: `git log <last-tag>..HEAD --oneline`.
2. Write `releases/vX.Y.Z.md` first — it's the complete technical record, and the
   GitHub notes are a distillation of it, not the other way around.
3. Write the GitHub release notes by extracting only what a user needs to know from
   step 2, in plain language.
4. Commit `releases/vX.Y.Z.md` to `main` alongside the version bump, before tagging.

## Scope

The `releases/` folder starts at `v1.0.0` — every release from `v1.0.0` onward gets a
file. Releases before `v1.0.0` don't get one added retroactively.

**Already-published GitHub release notes are never edited to match this format**,
including `v1.0.0`'s own — this guide governs how future release notes get *written*,
not a cleanup pass over what's already out. If an existing release's notes need a
correction for accuracy, that's a separate, explicit request, not something this guide
triggers on its own.
