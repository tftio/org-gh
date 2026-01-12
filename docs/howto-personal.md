# org-gh Personal Workflow

Track GitHub issues in org-mode files synced bidirectionally.

## Setup (One-Time)

### 1. Install org-gh

```sh
curl -fsSL https://raw.githubusercontent.com/tftio/org-gh/main/install.sh | sh
```

### 2. GitHub Token

Create a token at https://github.com/settings/tokens with `repo` scope.

```sh
# Option A: Environment variable
export GITHUB_TOKEN="ghp_xxxx"

# Option B: gh CLI (if already authenticated)
# org-gh will use `gh auth token` automatically
```

### 3. Load Elisp

**Using package.el (Emacs 29+):**

```elisp
(use-package org-gh
  :vc (:url "https://github.com/tftio/org-gh"
       :lisp-dir "elisp"))
```

**Or manual installation:**

```elisp
(add-to-list 'load-path "~/.local/share/org-gh/elisp")
(require 'org-gh)
```

## Starting a New Project

Create `~/org/{repo}-notes.org`:

```org
#+TITLE: myproject Notes
#+GH_REPO: owner/myproject

* TODO Implement user authentication
:PROPERTIES:
:END:

First pass at auth - need OAuth and session management.

* TODO Fix database connection pooling
:PROPERTIES:
:LABELS: bug, database
:ASSIGNEE: myusername
:END:

Connections exhausted under load.
```

Then sync:

```
C-c g s    (or M-x org-gh-sync)
```

This creates GitHub issues and adds `GH_ISSUE` and `GH_URL` properties to each heading.

## Daily Workflow

### Writing New Issues

Just add a TODO heading and sync:

```org
* TODO New feature idea
:PROPERTIES:
:LABELS: enhancement
:END:

Description goes in the body.
```

`C-c g s` creates the issue on GitHub.

### Pulling Changes from GitHub

Someone closes an issue or adds labels on GitHub:

```
C-c g p    (pull)
```

Your org file updates: `TODO` becomes `DONE`, labels/assignees sync.

### Pushing Local Changes

Mark something DONE locally or edit the description:

```
C-c g s    (sync handles both directions)
```

### Quick Actions

| Key     | Command              | Description                    |
|---------|----------------------|--------------------------------|
| C-c g s | `org-gh-sync`        | Bidirectional sync             |
| C-c g p | `org-gh-pull`        | Pull GitHub → org              |
| C-c g P | `org-gh-push-heading`| Push heading at point          |
| C-c g S | `org-gh-status`      | Show sync status               |
| C-c g i | `org-gh-init`        | Initialize file for repo       |
| C-c g b | `org-gh-browse`      | Open issue at point in browser |
| C-c g u | `org-gh-unlink`      | Remove GH link from heading    |

## Org Structure → GitHub Mapping

| Org                     | GitHub            |
|-------------------------|-------------------|
| Headline text           | Issue title       |
| Body under headline     | Issue body        |
| `TODO` / `DONE`         | Open / Closed     |
| `:LABELS:` property     | Labels (comma-separated) |
| `:ASSIGNEE:` property   | Assignees (comma-separated) |
| `:GH_ISSUE:` property   | Issue number (auto-set) |
| `:GH_URL:` property     | Issue URL (auto-set) |

## Example Workflow

```org
#+TITLE: acme-api Notes
#+GH_REPO: mycompany/acme-api

* DONE Set up CI pipeline
:PROPERTIES:
:GH_ISSUE: 1
:GH_URL: https://github.com/mycompany/acme-api/issues/1
:END:

GitHub Actions with test + deploy.

* TODO [#A] Fix rate limiting bug
:PROPERTIES:
:GH_ISSUE: 12
:GH_URL: https://github.com/mycompany/acme-api/issues/12
:LABELS: bug, urgent
:ASSIGNEE: jfb
:END:

Rate limiter not resetting after window expires.
Need to check Redis TTL logic.

* TODO Add OpenAPI docs
:PROPERTIES:
:LABELS: documentation
:END:

Generate from route definitions.
```

## Conflict Resolution

If both sides changed the same field since last sync:

- **Title/Body**: Org wins (org is the authoring surface)
- **State**: Prompts you to choose
- **Assignees**: GitHub wins
- **Labels**: Union merge (keeps both)

Use `--force` flag to make org win all conflicts:

```sh
org-gh sync ~/org/myproject-notes.org --force
```

## CLI Reference

```sh
# Initialize new file
org-gh init ~/org/newproject-notes.org owner/repo

# Sync (bidirectional)
org-gh sync ~/org/myproject-notes.org

# Pull only (GitHub → org)
org-gh pull ~/org/myproject-notes.org

# Push only (org → GitHub)
org-gh push ~/org/myproject-notes.org

# Check status
org-gh status ~/org/myproject-notes.org

# Dry run (see what would happen)
org-gh sync ~/org/myproject-notes.org --dry-run
```

## Tips

1. **First sync initializes state** - The first sync records current values as the baseline for three-way diff.

2. **Properties drawer is required** - Even if empty, each issue heading needs `:PROPERTIES:` / `:END:`.

3. **Matching existing issues** - If you add a heading with the same title as an existing GitHub issue, org-gh links them instead of creating a duplicate.

4. **State file** - Sync state is stored in `{filename}.org-gh.json` alongside your org file. Don't delete it or you'll lose the baseline for conflict detection.
