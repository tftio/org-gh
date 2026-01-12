# org-gh

Bidirectional sync between org-mode and GitHub Issues.

## Installation

### Quick install (macOS Apple Silicon)

```sh
curl -fsSL https://raw.githubusercontent.com/tftio/org-gh/master/install.sh | sh
```

### From source

```sh
cargo install --git https://github.com/tftio/org-gh
```

## Emacs Setup

### Using package.el (Emacs 29+)

```elisp
(package-vc-install
 '(org-gh :url "https://github.com/tftio/org-gh"
          :lisp-dir "elisp"))
```

Or with use-package:

```elisp
(use-package org-gh
  :vc (:url "https://github.com/tftio/org-gh"
       :lisp-dir "elisp"))
```

### Using straight.el

```elisp
(straight-use-package
 '(org-gh :type git
          :host github
          :repo "tftio/org-gh"
          :files ("elisp/*.el")))
```

### Manual installation

```elisp
(add-to-list 'load-path "~/.local/share/org-gh/elisp")
(require 'org-gh)
```

## Usage

### Initialize a file

```sh
org-gh init --repo owner/repo todo.org
```

Or in Emacs: `M-x org-gh-init`

### Sync

```sh
org-gh sync todo.org
```

Or in Emacs: `C-c g s` (with `org-gh-mode` active)

### Keybindings

When `org-gh-mode` is active (auto-enabled for files with `#+GH_REPO:`):

| Key       | Command              | Description               |
|-----------|----------------------|---------------------------|
| `C-c g s` | `org-gh-sync`        | Bidirectional sync        |
| `C-c g p` | `org-gh-pull`        | Pull from GitHub          |
| `C-c g P` | `org-gh-push-heading`| Push current heading      |
| `C-c g S` | `org-gh-status`      | Show sync status          |
| `C-c g i` | `org-gh-init`        | Initialize file           |
| `C-c g b` | `org-gh-browse`      | Open issue in browser     |
| `C-c g u` | `org-gh-unlink`      | Remove sync link          |

## How it works

- Org headings become GitHub issues
- Heading title → Issue title
- Heading body → Issue body
- `TODO`/`DONE` → Open/Closed state
- Properties drawer stores `:GH_ISSUE:` and `:GH_URL:`
- Three-way merge detects conflicts

## Configuration

Set `GITHUB_TOKEN` environment variable, or use `gh auth login`.

## License

MIT
