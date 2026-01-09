# org-gh: Bidirectional Org-mode / GitHub Issues Sync

**Created**: 2026-01-09
**Author**: JFB + Claude
**Status**: Design Spec

---

## Overview

`org-gh` is a Rust CLI tool that provides bidirectional synchronization between org-mode files and GitHub Issues. It enables a workflow where:

- You write and manage work items in Emacs org-mode (local-first, text-based)
- Team members see status and collaborate via GitHub Issues (visibility, PR integration)
- Changes flow both directions automatically

### Goals

1. **Local-first authoring**: Org-mode is the primary writing surface
2. **Visibility without friction**: Team sees progress in GitHub without you updating two systems
3. **PR integration**: Issues link to PRs, closures reflected in org
4. **Minimal conflicts**: Smart merge strategy that rarely requires intervention
5. **Automation-friendly**: Runs unattended on save/load hooks

### Non-Goals (v1)

- GitHub Projects board sync (Issues only for v1)
- Nested issue hierarchies (flat structure for v1)
- Real-time sync (batch sync on trigger, not live)
- Multi-repo sync in single org file

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         org-gh CLI                              │
├─────────────────────────────────────────────────────────────────┤
│  Commands:                                                      │
│    org-gh init <file> --repo <owner/repo>                       │
│    org-gh push <file> [--force]                                 │
│    org-gh pull <file>                                           │
│    org-gh sync <file> [--force] [--dry-run]                     │
│    org-gh status <file>                                         │
│    org-gh unlink <file> <heading>                               │
└─────────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
   │  Org Parser │     │  Sync Engine│     │ GitHub API  │
   │  (orgize)   │     │             │     │ (octocrab)  │
   └─────────────┘     └─────────────┘     └─────────────┘
          │                   │                   │
          ▼                   ▼                   ▼
   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
   │  Org Writer │     │ Sync State  │     │   REST +    │
   │ (starsector)│     │   Store     │     │  GraphQL    │
   └─────────────┘     └─────────────┘     └─────────────┘
```

### Components

| Component | Library | Purpose |
|-----------|---------|---------|
| CLI | clap | Command-line interface |
| Org Parser | orgize | Parse org files, extract headings/properties |
| Org Writer | starsector or custom | Write back with minimal diff |
| GitHub Client | octocrab | REST API for Issues |
| Sync State | serde + JSON | Track last-known state for conflict detection |
| Config | toml + serde | Authentication, defaults |

---

## Data Model

### Org File Structure

```org
#+TITLE: Nucleus Roadmap
#+GH_REPO: workhelix/nucleus
#+GH_LABEL_PREFIX: roadmap-
#+STARTUP: overview

* Tier 1: Unblock Delegation                                    :tier1:
Enabling other team members to run customer operations.

** TODO Cloud-based deployment runner
:PROPERTIES:
:GH_ISSUE: 43
:GH_URL: https://github.com/workhelix/nucleus/issues/43
:ASSIGNEE: jfb
:LABELS: infrastructure, priority-high
:CREATED: 2026-01-09
:UPDATED: 2026-01-09T15:30:00Z
:END:

Remove local machine dependency for ~customer deploy~.

Currently requires:
- AWS credentials configured per-customer
- Age private key for secrets decryption
- Tailscale connection

*** DONE Design CI workflow
CLOSED: [2026-01-15 Wed 14:30]

*** TODO Implement secrets handling

** DOING Implement customer status/test
:PROPERTIES:
:GH_ISSUE: 44
:GH_URL: https://github.com/workhelix/nucleus/issues/44
:END:

Fill in stub commands for post-deploy validation.

:LOGBOOK:
- Comment by @teammate [2026-01-10]: Can we add health check for Redis too?
- PR #52 linked [2026-01-12]
:END:

** TODO Drift detection system

This heading has no :GH_ISSUE: property, so push will create a new issue.
```

### File-Level Properties

| Property | Required | Description |
|----------|----------|-------------|
| `#+GH_REPO:` | Yes | GitHub repository (owner/repo format) |
| `#+GH_LABEL_PREFIX:` | No | Prefix added to all labels from this file |
| `#+GH_DEFAULT_LABELS:` | No | Labels applied to all new issues |

### Heading-Level Properties

| Property | Set By | Description |
|----------|--------|-------------|
| `:GH_ISSUE:` | Sync | GitHub issue number (assigned on first push) |
| `:GH_URL:` | Sync | Full URL to issue (convenience) |
| `:ASSIGNEE:` | User/Sync | GitHub username (without @) |
| `:LABELS:` | User/Sync | Comma-separated label names |
| `:CREATED:` | Sync | ISO date when issue was created |
| `:UPDATED:` | Sync | ISO timestamp of last sync |

### TODO States Mapping

| Org State | GitHub State | Notes |
|-----------|--------------|-------|
| `TODO` | open | Default open state |
| `DOING` | open + `in-progress` label | Optional: configurable label |
| `BLOCKED` | open + `blocked` label | Optional: configurable label |
| `DONE` | closed | Closed as completed |
| `CANCELLED` | closed | Closed as not planned |

Custom TODO sequences supported via org-mode's `#+SEQ_TODO:`.

### Sync State File

Stored at `<orgfile>.org-gh.json` (sibling to org file):

```json
{
  "version": 1,
  "repo": "workhelix/nucleus",
  "last_sync": "2026-01-09T15:30:00Z",
  "items": {
    "43": {
      "org_heading_id": "cloud-based-deployment-runner",
      "title": "Cloud-based deployment runner",
      "body_hash": "sha256:abc123...",
      "state": "open",
      "assignee": "jfb",
      "labels": ["infrastructure", "priority-high"],
      "gh_updated_at": "2026-01-09T14:00:00Z",
      "org_updated_at": "2026-01-09T15:30:00Z"
    },
    "44": {
      "org_heading_id": "implement-customer-status-test",
      "title": "Implement customer status/test",
      "body_hash": "sha256:def456...",
      "state": "open",
      "assignee": null,
      "labels": [],
      "gh_updated_at": "2026-01-10T09:00:00Z",
      "org_updated_at": "2026-01-09T15:30:00Z"
    }
  },
  "pending_creates": [
    {
      "org_heading_id": "drift-detection-system",
      "title": "Drift detection system"
    }
  ]
}
```

### Heading Identification

Headings are identified by a stable ID derived from:
1. `:CUSTOM_ID:` property if present
2. Otherwise: slugified heading text at time of first sync

This allows renaming headings without breaking the link to GitHub issues.

---

## Sync Algorithm

### Overview

```
SYNC(org_file):
    org_items = parse(org_file)
    gh_issues = fetch_issues(repo)
    base_state = load_sync_state(org_file)

    actions = []
    new_state = {}

    # Match org items to GitHub issues
    for item in org_items:
        if item.gh_issue:
            gh = gh_issues[item.gh_issue]
            base = base_state[item.gh_issue]
            action = reconcile(item, gh, base)
            actions.append(action)
        else:
            actions.append(CreateIssue(item))

    # Check for issues in GitHub not in org (optional)
    for issue in gh_issues:
        if issue not in org_items and issue in base_state:
            # Issue was in org but removed - close it? warn?
            actions.append(Warn("Issue #{} removed from org"))

    # Execute actions
    if dry_run:
        print(actions)
    else:
        execute(actions)
        save_sync_state(new_state)
```

### Reconciliation Logic

```
RECONCILE(org_item, gh_issue, base):
    org_changes = diff(base, org_item)
    gh_changes = diff(base, gh_issue)

    if empty(org_changes) and empty(gh_changes):
        return NoOp()

    merged = {}
    conflicts = []

    for field in [title, body, state, assignee, labels]:
        org_changed = field in org_changes
        gh_changed = field in gh_changes

        if org_changed and gh_changed:
            # Both changed - apply resolution strategy
            merged[field] = resolve(field, org_item, gh_issue, base)
            if needs_prompt(field):
                conflicts.append(field)
        elif org_changed:
            merged[field] = org_item[field]
        elif gh_changed:
            merged[field] = gh_issue[field]
        else:
            merged[field] = base[field]

    return Update(merged, conflicts)
```

### Conflict Resolution Strategy

| Field | Resolution | Rationale |
|-------|------------|-----------|
| `title` | Org wins | Org is the authoring surface |
| `body` | Org wins | Org is the authoring surface |
| `state` | Prompt (interactive) or Org wins (--force) | Both sides legitimately change state |
| `assignee` | GitHub wins | Assignment often happens in triage |
| `labels` | Union (merge both) | Additive, labels rarely conflict |

### Append-Only Fields (GitHub → Org)

These fields are never pushed from org, only pulled:

- **Comments**: Appended to `:LOGBOOK:` drawer
- **PR links**: Appended to `:LOGBOOK:` drawer
- **Closed-by PR**: Sets DONE state, adds PR reference

```org
:LOGBOOK:
- Comment by @teammate [2026-01-10T09:15:00Z]:
  Can we add health check for Redis too?
- PR #52 linked [2026-01-12T14:30:00Z]
- Closed by PR #52 [2026-01-15T10:00:00Z]
:END:
```

---

## CLI Interface

### Commands

#### `org-gh init`

Initialize sync for an org file.

```bash
org-gh init roadmap.org --repo workhelix/nucleus

# Creates:
# - Adds #+GH_REPO: header if missing
# - Creates roadmap.org.org-gh.json with empty state
# - Validates GitHub access
```

#### `org-gh push`

Push org changes to GitHub.

```bash
# Normal push - creates/updates issues from org
org-gh push roadmap.org

# Force push - org wins all conflicts, no prompts
org-gh push roadmap.org --force

# Dry run - show what would happen
org-gh push roadmap.org --dry-run
```

#### `org-gh pull`

Pull GitHub changes to org.

```bash
# Normal pull - updates org from GitHub
org-gh pull roadmap.org

# Pull never overwrites org content (title/body)
# Only updates: state, assignee, labels, comments
```

#### `org-gh sync`

Bidirectional sync.

```bash
# Interactive sync - prompts on conflicts
org-gh sync roadmap.org

# Force sync - org wins all conflicts
org-gh sync roadmap.org --force

# Dry run - preview changes
org-gh sync roadmap.org --dry-run

# Verbose - show all operations
org-gh sync roadmap.org --verbose
```

#### `org-gh status`

Show sync status.

```bash
org-gh status roadmap.org

# Output:
# Repository: workhelix/nucleus
# Last sync: 2026-01-09 15:30:00 UTC
#
# Synced items: 12
# Pending creates: 2 (new headings without GH_ISSUE)
#
# Local changes (not pushed):
#   - #43: title changed
#   - #45: marked DONE
#
# Remote changes (not pulled):
#   - #44: 2 new comments
#   - #46: closed by PR #52
```

#### `org-gh unlink`

Remove sync link without closing issue.

```bash
org-gh unlink roadmap.org "Cloud-based deployment runner"

# Removes GH_ISSUE property, removes from sync state
# Issue remains open in GitHub
```

### Global Options

```bash
org-gh --config ~/.config/org-gh/config.toml  # Custom config path
org-gh --token ghp_xxx                         # Override GitHub token
org-gh --quiet                                 # Suppress non-error output
org-gh --json                                  # Output as JSON (for scripting)
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (API failure, parse error, etc.) |
| 2 | Conflicts requiring resolution (interactive mode) |
| 3 | Configuration error |

---

## Configuration

### Config File

Located at `~/.config/org-gh/config.toml`:

```toml
[github]
# Authentication (in order of precedence):
# 1. --token CLI flag
# 2. GITHUB_TOKEN environment variable
# 3. gh CLI auth (gh auth token)
# 4. This config file
token = "ghp_xxxxxxxxxxxx"

# Default repository (can be overridden per-file)
default_repo = "workhelix/nucleus"

[sync]
# State mapping for DOING/BLOCKED states
doing_label = "in-progress"
blocked_label = "blocked"

# Auto-apply these labels to all new issues
default_labels = ["org-gh-managed"]

# Conflict resolution defaults
# Options: "prompt", "org-wins", "github-wins"
title_conflict = "org-wins"
body_conflict = "org-wins"
state_conflict = "prompt"
assignee_conflict = "github-wins"

[org]
# Custom TODO keywords to recognize
todo_keywords = ["TODO", "DOING", "BLOCKED", "WAITING"]
done_keywords = ["DONE", "CANCELLED", "WONTFIX"]

[sync_state]
# Where to store sync state
# Options: "sibling" (next to org file), "xdg" (~/.local/share/org-gh/)
location = "sibling"
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `GITHUB_TOKEN` | GitHub personal access token |
| `ORG_GH_CONFIG` | Path to config file |
| `ORG_GH_DEBUG` | Enable debug logging |

### GitHub Token Scopes

Required scopes for the GitHub token:
- `repo` (full control of private repositories)
- Or for public repos only: `public_repo`

---

## Emacs Integration

### Package: `org-gh.el`

```elisp
;;; org-gh.el --- Emacs integration for org-gh sync -*- lexical-binding: t; -*-

;;; Commentary:
;; Thin wrapper around org-gh CLI for Emacs integration.

;;; Code:

(defgroup org-gh nil
  "Org-mode GitHub sync."
  :group 'org)

(defcustom org-gh-executable "org-gh"
  "Path to org-gh executable."
  :type 'string
  :group 'org-gh)

(defcustom org-gh-auto-push-on-save nil
  "If non-nil, automatically push on save."
  :type 'boolean
  :group 'org-gh)

(defcustom org-gh-auto-pull-on-open t
  "If non-nil, automatically pull when opening synced files."
  :type 'boolean
  :group 'org-gh)

(defcustom org-gh-force-on-auto-sync t
  "If non-nil, use --force for automatic syncs (no prompts)."
  :type 'boolean
  :group 'org-gh)

(defun org-gh--is-synced-file-p ()
  "Return non-nil if current buffer is an org-gh synced file."
  (and (eq major-mode 'org-mode)
       (buffer-file-name)
       (save-excursion
         (goto-char (point-min))
         (re-search-forward "^#\\+GH_REPO:" nil t))))

(defun org-gh--run (command &rest args)
  "Run org-gh COMMAND with ARGS, returning (exit-code . output)."
  (with-temp-buffer
    (let* ((exit-code (apply #'call-process org-gh-executable nil t nil
                             command args)))
      (cons exit-code (buffer-string)))))

(defun org-gh--run-async (command &rest args)
  "Run org-gh COMMAND with ARGS asynchronously in compilation buffer."
  (let ((cmd (mapconcat #'shell-quote-argument
                        (cons org-gh-executable (cons command args))
                        " ")))
    (compile cmd)))

;;;###autoload
(defun org-gh-push (&optional force)
  "Push current org buffer to GitHub.
With prefix arg FORCE, use --force flag."
  (interactive "P")
  (unless (org-gh--is-synced-file-p)
    (user-error "Current buffer is not an org-gh synced file"))
  (save-buffer)
  (let* ((file (buffer-file-name))
         (args (if force
                   (list file "--force")
                 (list file)))
         (result (apply #'org-gh--run "push" args)))
    (if (= (car result) 0)
        (message "org-gh: pushed successfully")
      (message "org-gh push failed: %s" (cdr result)))))

;;;###autoload
(defun org-gh-pull ()
  "Pull GitHub changes to current org buffer."
  (interactive)
  (unless (org-gh--is-synced-file-p)
    (user-error "Current buffer is not an org-gh synced file"))
  (let* ((file (buffer-file-name))
         (result (org-gh--run "pull" file)))
    (if (= (car result) 0)
        (progn
          (revert-buffer t t)
          (message "org-gh: pulled successfully"))
      (message "org-gh pull failed: %s" (cdr result)))))

;;;###autoload
(defun org-gh-sync (&optional force)
  "Bidirectional sync current org buffer with GitHub.
With prefix arg FORCE, use --force flag."
  (interactive "P")
  (unless (org-gh--is-synced-file-p)
    (user-error "Current buffer is not an org-gh synced file"))
  (save-buffer)
  (let ((file (buffer-file-name)))
    (if force
        ;; Force mode - run synchronously
        (let ((result (org-gh--run "sync" file "--force")))
          (revert-buffer t t)
          (if (= (car result) 0)
              (message "org-gh: synced successfully")
            (message "org-gh sync failed: %s" (cdr result))))
      ;; Interactive mode - use compilation buffer for prompts
      (org-gh--run-async "sync" file))))

;;;###autoload
(defun org-gh-status ()
  "Show sync status for current org buffer."
  (interactive)
  (unless (org-gh--is-synced-file-p)
    (user-error "Current buffer is not an org-gh synced file"))
  (let* ((file (buffer-file-name))
         (result (org-gh--run "status" file)))
    (with-output-to-temp-buffer "*org-gh status*"
      (princ (cdr result)))))

;;;###autoload
(defun org-gh-init (repo)
  "Initialize org-gh sync for current buffer with REPO."
  (interactive "sGitHub repository (owner/repo): ")
  (unless (eq major-mode 'org-mode)
    (user-error "Current buffer is not an org-mode buffer"))
  (unless (buffer-file-name)
    (user-error "Buffer must be saved to a file first"))
  (save-buffer)
  (let* ((file (buffer-file-name))
         (result (org-gh--run "init" file "--repo" repo)))
    (if (= (car result) 0)
        (progn
          (revert-buffer t t)
          (message "org-gh: initialized for %s" repo))
      (message "org-gh init failed: %s" (cdr result)))))

;;;###autoload
(defun org-gh-browse-issue ()
  "Open the GitHub issue for heading at point in browser."
  (interactive)
  (let ((url (org-entry-get nil "GH_URL")))
    (if url
        (browse-url url)
      (user-error "No GH_URL property on this heading"))))

;; Auto-sync hooks

(defun org-gh--after-save-hook ()
  "Hook to run after saving an org-gh synced file."
  (when (and org-gh-auto-push-on-save
             (org-gh--is-synced-file-p))
    (let ((args (if org-gh-force-on-auto-sync '("--force") nil)))
      (apply #'org-gh--run "push" (buffer-file-name) args)
      (message "org-gh: auto-pushed"))))

(defun org-gh--find-file-hook ()
  "Hook to run when opening an org-gh synced file."
  (when (and org-gh-auto-pull-on-open
             (org-gh--is-synced-file-p))
    (let ((result (org-gh--run "pull" (buffer-file-name))))
      (when (= (car result) 0)
        (revert-buffer t t)
        (message "org-gh: auto-pulled")))))

;;;###autoload
(define-minor-mode org-gh-mode
  "Minor mode for org-gh integration."
  :lighter " GH"
  :group 'org-gh
  (if org-gh-mode
      (progn
        (add-hook 'after-save-hook #'org-gh--after-save-hook nil t)
        (add-hook 'find-file-hook #'org-gh--find-file-hook nil t))
    (remove-hook 'after-save-hook #'org-gh--after-save-hook t)
    (remove-hook 'find-file-hook #'org-gh--find-file-hook t)))

;; Keybindings

(defvar org-gh-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "C-c g p") #'org-gh-push)
    (define-key map (kbd "C-c g l") #'org-gh-pull)
    (define-key map (kbd "C-c g s") #'org-gh-sync)
    (define-key map (kbd "C-c g t") #'org-gh-status)
    (define-key map (kbd "C-c g o") #'org-gh-browse-issue)
    map)
  "Keymap for `org-gh-mode'.")

(provide 'org-gh)
;;; org-gh.el ends here
```

### Installation

```elisp
;; In init.el or use-package

;; Add to load path
(add-to-list 'load-path "~/path/to/org-gh/emacs")

;; Load and configure
(require 'org-gh)

(setq org-gh-executable "~/.cargo/bin/org-gh")
(setq org-gh-auto-push-on-save t)
(setq org-gh-auto-pull-on-open t)

;; Enable for all org files (or add to specific files)
(add-hook 'org-mode-hook
          (lambda ()
            (when (org-gh--is-synced-file-p)
              (org-gh-mode 1))))
```

---

## Implementation Plan

### Phase 1: Core Parsing (Week 1)

1. **Project setup**
   - Cargo workspace with `org-gh` binary and `org-gh-lib` library
   - CI with tests, clippy, rustfmt

2. **Org parser**
   - Parse org file using orgize
   - Extract syncable headings (top 2 levels by default)
   - Extract properties (GH_ISSUE, ASSIGNEE, LABELS)
   - Map TODO state

3. **Org writer**
   - Update properties on existing headings
   - Preserve formatting, comments, whitespace
   - Append to LOGBOOK drawer

### Phase 2: GitHub Integration (Week 2)

4. **GitHub client**
   - Authentication (token, gh CLI)
   - Fetch issues with pagination
   - Create/update/close issues
   - Fetch comments and PR links

5. **Sync state**
   - Load/save JSON state file
   - Compute diffs from base state
   - Track pending creates

### Phase 3: Sync Engine (Week 3)

6. **Reconciliation**
   - Three-way diff (org, github, base)
   - Field-level conflict detection
   - Merge strategies per field

7. **CLI commands**
   - `init`, `push`, `pull`, `sync`, `status`
   - Flags: --force, --dry-run, --verbose, --json
   - Interactive conflict prompts

### Phase 4: Polish (Week 4)

8. **Emacs integration**
   - Package with autoloads
   - Auto-sync hooks
   - Keybindings

9. **Documentation**
   - README with examples
   - Config file reference
   - Troubleshooting guide

10. **Testing**
    - Unit tests for parser, writer, sync logic
    - Integration tests with GitHub API (mock or sandbox repo)

---

## Project Structure

```
org-gh/
├── Cargo.toml
├── README.md
├── LICENSE
├── .github/
│   └── workflows/
│       └── ci.yml
├── src/
│   ├── main.rs                 # CLI entry (clap)
│   ├── lib.rs                  # Library exports
│   ├── cli/
│   │   ├── mod.rs
│   │   ├── init.rs
│   │   ├── push.rs
│   │   ├── pull.rs
│   │   ├── sync.rs
│   │   └── status.rs
│   ├── org/
│   │   ├── mod.rs
│   │   ├── parser.rs           # Orgize wrapper
│   │   ├── writer.rs           # Minimal-diff writes
│   │   ├── model.rs            # OrgItem, OrgFile
│   │   └── todo_state.rs       # TODO keyword mapping
│   ├── github/
│   │   ├── mod.rs
│   │   ├── client.rs           # Octocrab wrapper
│   │   ├── issues.rs           # Issue operations
│   │   └── model.rs            # GhIssue, GhComment
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── state.rs            # Sync state file
│   │   ├── diff.rs             # Three-way diff
│   │   ├── merge.rs            # Field-level merge
│   │   └── engine.rs           # Main sync orchestration
│   ├── config.rs               # TOML config
│   └── error.rs                # Error types
├── emacs/
│   └── org-gh.el               # Emacs package
└── tests/
    ├── org_parser_test.rs
    ├── org_writer_test.rs
    ├── sync_test.rs
    └── fixtures/
        ├── simple.org
        ├── with_properties.org
        └── complex.org
```

---

## Cargo.toml

```toml
[package]
name = "org-gh"
version = "0.1.0"
edition = "2021"
authors = ["JFB"]
description = "Bidirectional sync between org-mode and GitHub Issues"
license = "MIT"
repository = "https://github.com/jfb/org-gh"
keywords = ["org-mode", "github", "sync", "emacs"]
categories = ["command-line-utilities", "development-tools"]

[dependencies]
# CLI
clap = { version = "4", features = ["derive"] }

# Async runtime
tokio = { version = "1", features = ["full"] }

# Org parsing
orgize = "0.10"

# GitHub API
octocrab = "0.44"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"

# Utilities
thiserror = "2"
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"                    # For body hashing
directories = "5"               # XDG paths
dialoguer = "0.11"              # Interactive prompts
console = "0.15"                # Terminal colors

[dev-dependencies]
tempfile = "3"
wiremock = "0.6"                # Mock HTTP for tests
```

---

## Open Questions

1. **Subtask handling**: Currently treating subtasks as just org structure (not synced). Should checklist items (`- [ ] foo`) in issue body round-trip?

2. **Label namespacing**: If multiple org files sync to same repo, labels could collide. Use prefix per file?

3. **Rate limiting**: GitHub API has rate limits. Cache aggressively? Batch requests?

4. **Large files**: What's the performance ceiling? 100 items? 1000?

5. **Offline mode**: Queue changes when offline, apply when online?

---

## Success Criteria

v1.0 is ready when:

- [ ] Can create new issues from org headings
- [ ] Can update issues when org changes
- [ ] Can close issues when org marks DONE
- [ ] Pull updates state, assignee, comments to org
- [ ] Conflicts detected and resolved (prompt or force)
- [ ] Emacs integration works (save/load hooks)
- [ ] Documentation complete
- [ ] Tested with real repo (this roadmap!)
