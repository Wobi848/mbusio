# Working on mbusio

Talk to the maintainer in German. Everything on GitHub is in English.

## Git and GitHub
- Commits, branches, PRs, issues, tags and release notes in English. Older commits stay as they are.
- No AI attribution: no `Co-Authored-By`, no "Generated with".
- One short branch per topic, named after the topic (`design-polish`). Never commit directly to `main`.
- Stage your own files by name, never `git add -A`. Another AI session may be working in the repo.
- Separate commits per concern; the subject says what changes.
- Merge to `main` only after the maintainer says so, and only with green CI.

## Releases
- A `v*` tag builds and publishes a release. Create tags only on explicit request.
- Same for npm. Windows builds only on explicit request.

## Before saying "done"
- Run `cargo test` and the UI tests (`tests/ui/README.md`).
- Say what could not be checked (real hardware, broker).
- Never remove a feature without showing where it lives on in the new version.
- Bigger changes: concept in `docs/` first, then code. New concepts in English.
- User-facing changes go in `CHANGELOG.md` under "Unreleased", in English.

## Languages
- New interface and server texts need every language; the tests fail otherwise (`docs/concept-languages.md`).
- A language enters the menu only after review by someone from the trade. The same goes for
  publishing the Node-RED package with it, because Node-RED has no menu to hide a language.
