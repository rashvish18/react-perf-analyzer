# Changelog

All notable changes to `react-perf-analyzer` are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).  
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [0.5.3] — 2026-04-15

### Added
- **`--format ai-prompt`** — AI Prompt Mode: generates rich, self-contained markdown fix prompts for every file with issues. Paste into Claude, GitHub Copilot Chat, or Cursor to get guided fixes.
  - **Single-file mode**: one combined `.md` with all issues + annotated source
  - **Directory mode** (`--output ./dir/`): one `.md` per file + `index.md` orchestrator dashboard
- **Orchestrator `index.md`**: a paste-once agentic workflow file that:
  - Asks 4 intake questions (scope, severity, PR, skip list) — reply with numbers
  - Fixes files one at a time in priority order (security → high-impact → by module)
  - Auto-validates each fix (react-perf-analyzer + TypeScript + ESLint)
  - Updates live checkboxes as fixes are applied — acts as a progress tracker
  - Asks before committing/raising a PR with 4 options (commit / commit+PR / review diff / skip)
  - Shows "Type `Fix next` to continue" after each batch
  - Embeds source root + prompt directory paths so the AI always knows where to read/edit
- **Inline issue markers** in prompt source code: `// ← ⚠ Issue N` on affected lines
- **Token estimation** per file (`~N tokens`) so you can pick files that fit your AI context window
- **Rule explanation blurbs** (`rule_why_blurb`) — every issue includes a plain-English "Why it hurts" section
- **Priority scoring** — files sorted by `critical×100 + high×10 + medium×1` so security issues always appear first

### Improved
- **`no_hardcoded_secret_in_jsx` false positive reduction** — three new filters in `looks_like_secret()`:
  - Strings containing spaces (UI copy / error messages) are no longer flagged
  - Word-only strings (`[a-zA-Z0-9_-]`) with long alphabetic runs (camelCase key names, feature flag IDs) are excluded
  - Minimum length check remains at 12 characters
- **`--output` help text** now explicitly describes directory mode and the trailing `/` convention

### Fixed
- Double-slash in embedded prompt directory path (`/tmp/dir//`) — trimmed correctly

---

## [0.5.2] — 2026-03-20

### Fixed
- `--only-changed`: canonicalize scan root before comparing against git-changed paths (fixes false "no files changed" on symlinked paths)

---

## [0.5.1] — 2026-03-10

### Added
- `--baseline` flag: suppress known issues so CI only fails on new regressions
- SARIF output (`--format sarif`) for GitHub/GitLab inline PR annotations
- HTML report: self-contained single-file with stat tiles, per-rule filter cards, top-10 bar chart, collapsible issue table, real-time filename search

### Improved
- Performance: parallel file analysis pipeline (Rayon)
- `--only-changed` pre-commit mode — scans only git-modified files (<10 ms on large repos)

---

## [0.5.0] — 2026-02-15

### Added
- Initial public release
- 15 React performance rules (OXC AST-based)
- 5 React security rules: `no_unsafe_href`, `no_xss_via_jsx_prop`, `no_hardcoded_secret_in_jsx`, `no_dangerously_set_inner_html_unescaped`, `no_postmessage_wildcard`
- TOML custom rules DSL (no Rust required)
- `--external` flag to also invoke oxlint + cargo-audit
- Text, JSON, HTML, SARIF output formats
- CI templates: GitHub Actions, GitLab CI, Jenkins/Looper/Buildkite, pre-commit hook
