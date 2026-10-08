# Contributors in the About window

**About ▸ Contributors** credits everyone who contributed to Pixelwright, and **About ▸ Models** credits
the AI models named in `Co-Authored-By` trailers. The design comes from the upstream PhotoCraft project;
this page is the local copy of the decision.

## Decision

- Two files, both **compiled into the binary** by the UI crate's `build.rs` (nothing is read from
  disk or the network at run time; the web build has them too):
  - [`contributors/contributors.json`](../contributors/contributors.json): commit stats per GitHub
    username. **Generated** upstream by a script from the commit history (a private tool of the original authors). The upstream rows are kept as they were; the Pixelwright row (`game7ballJT`) was computed from this fork's git log (`git log --numstat`) and is refreshed by hand at release time.
  - [`contributors/people.toml`](../contributors/people.toml): the names people chose for
    themselves. **Each contributor adds only their own entry**, in their own PR.
- Each contributor gets one line: GitHub username (always), display name (opt-in, or their synced
  GitHub profile name), real name (only if they told us), merged PRs, commits, lines added, lines
  deleted, binary assets added, binary assets removed, first and last commit date.
- The list is shown as a **grab bag** (names flowing as text) or a **table**, in the same order. It
  sorts by name, PRs, commits, lines added, lines deleted, line delta, binary assets added/removed,
  first or last commit date. The name toggle cycles **Username → Display name → Real name**; a
  missing name falls back to `@username`. Alphabetical sorting is case-insensitive and ignores the `@`.

## In Pixelwright

- The About window is `DialogKind::About` in `crates/ui-egui/src/dialogs.rs` (Help ▸ About Pixelwright).
  Its tab is the dialog field `tab` (`about`, `contributors`, `models`), so agents can switch it with
  `ui.dialog.set`. Labels are translated like the rest of the shell (`tl!` + `crates/ui-egui/src/i18n/*.tsv`).

## Credit yourself

Add your entry to `contributors/people.toml` in a PR you commit yourself (every field optional):

```toml
[people.your-github-username]
real_name = "Your Name"
display_name = "Nickname"
sync_github_name = true   # use the name on your public GitHub profile as display name
```

Names show after the next refresh of `contributors/contributors.json` (a row needs `"people_entry": true`
to use the name from `people.toml`). Delete your entry to remove your names.

## Refreshing (maintainers)

Recount your commits with `git log --numstat` and update your row in `contributors/contributors.json`, then
commit it. Git author names and emails are never recorded.
