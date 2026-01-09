# Elisp Integration Design

## Overview

`org-gh.el` - A minor mode for bidirectional sync between org-mode and GitHub Issues.

## Decisions

| Question | Decision |
|----------|----------|
| Interaction model | Both buffer-centric and heading-centric commands |
| CLI execution | Async with callbacks using `make-process` |
| Package structure | Minor mode auto-activating for files with `#+GH_REPO:` |
| After push | Auto-update buffer with `:GH_ISSUE:` and `:GH_URL:` |
| Conflict handling | Minibuffer prompts (`y-or-n-p`) |
| Visual indicators | None (use `org-gh-status` command) |
| Key prefix | `C-c g` |
| CLI output format | S-expressions by default, `--json` flag for compatibility |

## File Structure

```
elisp/
  org-gh.el          ; Single-file package
```

## Dependencies

- Built-in only: `org.el`, `cl-lib.el`
- Emacs 27.1+ (for `make-process` improvements)
- No external packages required

## Customization Variables

- `org-gh-executable` - Path to `org-gh` binary (default: "org-gh")
- `org-gh-auto-mode` - Auto-enable for files with `#+GH_REPO:` (default: t)

## Commands & Keybindings

### Buffer commands (operate on entire file)

| Key | Command | Description |
|-----|---------|-------------|
| `C-c g s` | `org-gh-sync` | Bidirectional sync |
| `C-c g p` | `org-gh-pull` | Pull GitHub changes to org |
| `C-c g S` | `org-gh-status` | Show sync status in minibuffer |
| `C-c g i` | `org-gh-init` | Initialize file for a repo |

### Heading commands (operate on heading at point)

| Key | Command | Description |
|-----|---------|-------------|
| `C-c g P` | `org-gh-push-heading` | Push current heading to GitHub |
| `C-c g b` | `org-gh-browse` | Open issue in browser |
| `C-c g u` | `org-gh-unlink` | Remove sync link from heading |

## CLI Output Format

Default output is s-expressions (parseable with `read`):

```elisp
;; org-gh status
((file . "/path/to/file.org")
 (repo . "owner/repo")
 (synced . 3)
 (pending-push . 1)
 (pending-pull . 0))

;; org-gh push (after creating issue)
((action . created)
 (issue-number . 42)
 (url . "https://github.com/owner/repo/issues/42")
 (title . "My heading"))

;; org-gh sync (with conflicts)
((synced . 2)
 (conflicts . (((issue . 5)
                (field . title)
                (local . "New title")
                (remote . "Old title")))))
```

## Process Execution Flow

```
Command invoked (e.g., org-gh-sync)
    |
    v
Save buffer (prompt if unsaved)
    |
    v
make-process with CLI args
    |
    v
Collect stdout in process buffer
    |
    v
On exit: parse sexp with (read output)
    |
    +---> Success: update buffer, show message
    |
    +---> Error: display in minibuffer
```

## Minor Mode

```elisp
(define-minor-mode org-gh-mode
  "Minor mode for GitHub issue sync in org files."
  :lighter " GH"
  :keymap org-gh-mode-map)
```

Auto-activation via `org-mode-hook`:
- Scan buffer for `#+GH_REPO:` header
- If found, enable `org-gh-mode` automatically

## Elisp File Structure

```elisp
;;; org-gh.el --- Sync org-mode headings with GitHub Issues -*- lexical-binding: t -*-

;; Package-Requires: ((emacs "27.1") (org "9.0"))

;;; Customization
(defgroup org-gh nil ...)
(defcustom org-gh-executable "org-gh" ...)
(defcustom org-gh-auto-mode t ...)

;;; Internal variables
(defvar org-gh-mode-map (make-sparse-keymap))

;;; Process handling
(defun org-gh--run (args callback &optional error-callback) ...)
(defun org-gh--parse-output (output) ...)
(defun org-gh--handle-conflicts (conflicts callback) ...)

;;; Buffer manipulation
(defun org-gh--save-buffer () ...)
(defun org-gh--update-heading-properties (issue-number url) ...)
(defun org-gh--get-heading-at-point () ...)

;;; Commands
(defun org-gh-sync () ...)
(defun org-gh-pull () ...)
(defun org-gh-push-heading () ...)
(defun org-gh-status () ...)
(defun org-gh-init (repo) ...)
(defun org-gh-browse () ...)
(defun org-gh-unlink () ...)

;;; Minor mode
(defun org-gh--maybe-enable () ...)
(define-minor-mode org-gh-mode ...)
(add-hook 'org-mode-hook #'org-gh--maybe-enable)

(provide 'org-gh)
;;; org-gh.el ends here
```

## Installation

User adds `elisp/` to `load-path` and `(require 'org-gh)`.
