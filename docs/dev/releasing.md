# Releasing

A release is cut by pushing an annotated tag. `.github/workflows/release.yml` reacts to
the tag by regenerating `CHANGELOG.md` and publishing a GitHub release.

Nothing changelog-related happens on ordinary merges to `main` — the changelog only moves
when a tag is pushed.

## Prerequisites

```bash
cargo install git-cliff     # 2.13 or newer
gh auth status              # must be authenticated
export GITHUB_TOKEN=$(gh auth token)
```

`cliff.toml` uses the `github-keepachangelog` template, which resolves PR numbers and
author attribution through the GitHub API. Without a token you are capped at 60
unauthenticated requests per hour and will hit that quickly across repeated runs.

## 1. Decide the version

```bash
git-cliff --bumped-version
```

git-cliff derives the next version from the conventional commit types since the last tag:
any `feat:` bumps the minor, `fix:`-only bumps the patch, a breaking change bumps the
major. Use its answer unless you have a reason not to.

## 2. Preview

Both commands print to stdout and write nothing:

```bash
git-cliff --unreleased --tag vX.Y.Z    # the pending release section
git-cliff --latest --strip all         # what the GitHub release body will say
```

## 3. Prepare the release commit

```bash
git switch main && git pull

git-cliff --tag vX.Y.Z --output CHANGELOG.md

# Bump `version` in both crate manifests — they are versioned in lockstep:
#   crates/thesaurus/Cargo.toml
#   crates/thesaurus-integration-suite/Cargo.toml
cargo check                            # refreshes Cargo.lock

git add CHANGELOG.md Cargo.lock crates/*/Cargo.toml
git commit -m "chore(release): prepare for vX.Y.Z"
```

Add the paths explicitly. `git add -A` would sweep up untracked scratch files that are
not in `.gitignore`.

The commit message matters: `cliff.toml` skips `^chore\(release\): prepare for`, so this
commit stays out of the changelog. Generating `CHANGELOG.md` here rather than letting CI
do it also means the tagged commit contains its own changelog entry.

## 4. Tag and push

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin main --follow-tags
```

`--follow-tags` pushes the annotated tag alongside the commit, so the workflow fires once.

## 5. What CI does

On the tag push, `release.yml`:

1. Checks out `main` at full depth — git-cliff reads history and tags from the object
   database, and a shallow clone silently produces a near-empty changelog.
2. Regenerates `CHANGELOG.md` (full history, every release).
3. Generates `release-notes.md` (latest section only, ephemeral, never committed).
4. Commits `CHANGELOG.md` to `main` as `chore(changelog): update for vX.Y.Z [skip ci]`.
   If you generated it in step 3, the output is byte-identical, nothing is staged, and
   this step is a no-op.
5. Creates the GitHub release, titled with the tag, with `release-notes.md` as the body.

## Troubleshooting

**git-cliff panics with `Could not get github metadata`.** The GitHub API was unreachable.
git-cliff does not degrade gracefully here. Re-run, or fall back to
`git-cliff --offline --tag vX.Y.Z --output CHANGELOG.md`, which produces the same
structure without PR links or author attribution.

**`gh release create` fails with "release already exists".** Re-running the workflow on a
tag that already produced a release. Either delete the release first, or update it in
place with `gh release edit vX.Y.Z --notes-file release-notes.md`.

**The changelog came out nearly empty.** Almost always a shallow clone — check that
`fetch-depth: 0` is still set on the checkout step.

**A commit is missing from the changelog.** `cliff.toml` sets `filter_unconventional = true`
and skips the `doc`, `test`, `style`, `chore`, and `ci` types. Non-conventional commit
subjects are dropped entirely.
