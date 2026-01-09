;;; org-gh.el --- Sync org-mode headings with GitHub Issues -*- lexical-binding: t -*-

;; Author: org-gh contributors
;; Version: 0.1.0
;; Package-Requires: ((emacs "27.1") (org "9.0"))
;; Keywords: org, github, issues, sync
;; URL: https://github.com/tftio/org-gh

;; This file is not part of GNU Emacs.

;;; Commentary:

;; org-gh provides bidirectional sync between org-mode headings and GitHub
;; Issues.  It works by calling the `org-gh' CLI tool and parsing its
;; s-expression output.
;;
;; Usage:
;;   1. Initialize a file: M-x org-gh-init RET owner/repo RET
;;   2. Sync changes: C-c g s (or M-x org-gh-sync)
;;   3. Push a new heading: C-c g P (or M-x org-gh-push-heading)
;;
;; The minor mode `org-gh-mode' is automatically enabled for org files
;; that contain the #+GH_REPO: header.

;;; Code:

(require 'org)
(require 'cl-lib)

;;; Customization

(defgroup org-gh nil
  "Sync org-mode headings with GitHub Issues."
  :group 'org
  :prefix "org-gh-")

(defcustom org-gh-executable "org-gh"
  "Path to the org-gh executable."
  :type 'string
  :group 'org-gh)

(defcustom org-gh-auto-mode t
  "Automatically enable `org-gh-mode' for files with #+GH_REPO: header."
  :type 'boolean
  :group 'org-gh)

;;; Internal variables

(defvar org-gh-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "C-c g s") #'org-gh-sync)
    (define-key map (kbd "C-c g p") #'org-gh-pull)
    (define-key map (kbd "C-c g P") #'org-gh-push-heading)
    (define-key map (kbd "C-c g S") #'org-gh-status)
    (define-key map (kbd "C-c g i") #'org-gh-init)
    (define-key map (kbd "C-c g b") #'org-gh-browse)
    (define-key map (kbd "C-c g u") #'org-gh-unlink)
    map)
  "Keymap for `org-gh-mode'.")

(defvar-local org-gh--process nil
  "Current org-gh process for this buffer.")

;;; Process handling

(defun org-gh--run (args callback &optional error-callback)
  "Run org-gh with ARGS, call CALLBACK with parsed sexp on success.
ARGS should be a list of strings.  The --sexp flag is automatically added.
If ERROR-CALLBACK is provided, it's called with error message on failure."
  (let* ((buffer (current-buffer))
         (output-buffer (generate-new-buffer " *org-gh-output*"))
         (full-args (append (list org-gh-executable "--sexp") args)))
    ;; Save buffer before running
    (when (buffer-modified-p)
      (save-buffer))
    (make-process
     :name "org-gh"
     :buffer output-buffer
     :command full-args
     :sentinel
     (lambda (proc _event)
       (when (memq (process-status proc) '(exit signal))
         (with-current-buffer (process-buffer proc)
           (let ((output (buffer-string))
                 (exit-code (process-exit-status proc)))
             (kill-buffer (current-buffer))
             (with-current-buffer buffer
               (if (zerop exit-code)
                   (condition-case err
                       (let ((result (org-gh--parse-output output)))
                         (funcall callback result))
                     (error
                      (if error-callback
                          (funcall error-callback (format "Parse error: %s" err))
                        (message "org-gh: parse error: %s" err))))
                 (if error-callback
                     (funcall error-callback (string-trim output))
                   (message "org-gh error: %s" (string-trim output))))))))))))

(defun org-gh--parse-output (output)
  "Parse OUTPUT string from org-gh CLI as s-expression."
  (car (read-from-string output)))

(defun org-gh--get-file-path ()
  "Get the current buffer's file path, or error if not visiting a file."
  (or (buffer-file-name)
      (error "Buffer is not visiting a file")))

;;; Buffer manipulation

(defun org-gh--update-heading-properties (issue-number url)
  "Update current heading with ISSUE-NUMBER and URL properties."
  (org-entry-put nil "GH_ISSUE" (number-to-string issue-number))
  (org-entry-put nil "GH_URL" url))

(defun org-gh--get-heading-issue ()
  "Get the GH_ISSUE property of the heading at point, or nil."
  (let ((issue (org-entry-get nil "GH_ISSUE")))
    (when issue
      (string-to-number issue))))

(defun org-gh--get-heading-url ()
  "Get the GH_URL property of the heading at point, or nil."
  (org-entry-get nil "GH_URL"))

(defun org-gh--get-heading-title ()
  "Get the title of the heading at point."
  (org-get-heading t t t t))

;;; Commands - Buffer operations

;;;###autoload
(defun org-gh-init (repo)
  "Initialize the current file for syncing with GitHub REPO.
REPO should be in the format \"owner/repo\"."
  (interactive "sGitHub repository (owner/repo): ")
  (let ((file (org-gh--get-file-path)))
    (org-gh--run
     (list "init" "--repo" repo file)
     (lambda (_result)
       (revert-buffer t t t)
       (org-gh-mode 1)
       (message "Initialized for %s" repo))
     (lambda (err)
       (message "org-gh init failed: %s" err)))))

;;;###autoload
(defun org-gh-sync ()
  "Bidirectional sync between org file and GitHub."
  (interactive)
  (let ((file (org-gh--get-file-path)))
    (message "Syncing %s..." (file-name-nondirectory file))
    (org-gh--run
     (list "sync" file)
     (lambda (result)
       (revert-buffer t t t)
       (let ((pushed (length (alist-get 'pushed result)))
             (pulled (length (alist-get 'pulled result)))
             (conflicts (length (alist-get 'conflicts result))))
         (message "Sync complete: %d pushed, %d pulled%s"
                  pushed pulled
                  (if (> conflicts 0)
                      (format ", %d conflicts" conflicts)
                    ""))))
     (lambda (err)
       (message "org-gh sync failed: %s" err)))))

;;;###autoload
(defun org-gh-pull ()
  "Pull changes from GitHub to the org file."
  (interactive)
  (let ((file (org-gh--get-file-path)))
    (message "Pulling from GitHub...")
    (org-gh--run
     (list "pull" file)
     (lambda (result)
       (revert-buffer t t t)
       (let ((pulled (length (alist-get 'pulled result)))
             (conflicts (length (alist-get 'conflicts result))))
         (message "Pull complete: %d updated%s"
                  pulled
                  (if (> conflicts 0)
                      (format ", %d conflicts" conflicts)
                    ""))))
     (lambda (err)
       (message "org-gh pull failed: %s" err)))))

;;;###autoload
(defun org-gh-status ()
  "Show sync status for the current file."
  (interactive)
  (let ((file (org-gh--get-file-path)))
    (org-gh--run
     (list "status" file)
     (lambda (result)
       (let ((repo (alist-get 'repo result))
             (synced (alist-get 'synced-count result))
             (pending (length (alist-get 'pending-creates result)))
             (local (length (alist-get 'local-changes result)))
             (remote (length (alist-get 'remote-changes result))))
         (message "%s: %d synced, %d pending, %d local changes, %d remote changes"
                  repo synced pending local remote)))
     (lambda (err)
       (message "org-gh status failed: %s" err)))))

;;; Commands - Heading operations

;;;###autoload
(defun org-gh-push-heading ()
  "Push the current heading to GitHub.
If the heading has no GH_ISSUE property, creates a new issue.
Otherwise, updates the existing issue."
  (interactive)
  (unless (org-at-heading-p)
    (error "Not at an org heading"))
  (let* ((file (org-gh--get-file-path))
         (title (org-gh--get-heading-title)))
    (message "Pushing \"%s\"..." title)
    (org-gh--run
     (list "push" file)
     (lambda (result)
       (revert-buffer t t t)
       ;; Find our heading in the results
       (let* ((created (alist-get 'created result))
              (updated (alist-get 'updated result))
              (item (or (cl-find-if (lambda (i)
                                      (string= (alist-get 'title i) title))
                                    created)
                        (cl-find-if (lambda (i)
                                      (string= (alist-get 'title i) title))
                                    updated))))
         (if item
             (message "%s #%d: %s"
                      (alist-get 'action item)
                      (alist-get 'issue-number item)
                      title)
           (message "Pushed %s" title))))
     (lambda (err)
       (message "org-gh push failed: %s" err)))))

;;;###autoload
(defun org-gh-browse ()
  "Open the GitHub issue for the heading at point in a browser."
  (interactive)
  (unless (org-at-heading-p)
    (error "Not at an org heading"))
  (let ((url (org-gh--get-heading-url)))
    (if url
        (browse-url url)
      (error "Heading has no GH_URL property"))))

;;;###autoload
(defun org-gh-unlink ()
  "Remove the sync link from the heading at point.
The GitHub issue is not closed."
  (interactive)
  (unless (org-at-heading-p)
    (error "Not at an org heading"))
  (let* ((file (org-gh--get-file-path))
         (issue-num (org-gh--get-heading-issue)))
    (unless issue-num
      (error "Heading is not linked to a GitHub issue"))
    (when (yes-or-no-p (format "Unlink heading from issue #%d? " issue-num))
      (org-gh--run
       (list "unlink" file (number-to-string issue-num))
       (lambda (result)
         (revert-buffer t t t)
         (message "Unlinked from issue #%d" (alist-get 'issue-number result)))
       (lambda (err)
         (message "org-gh unlink failed: %s" err))))))

;;; Minor mode

(defun org-gh--maybe-enable ()
  "Enable `org-gh-mode' if buffer has #+GH_REPO: header."
  (when (and org-gh-auto-mode
             (derived-mode-p 'org-mode)
             (buffer-file-name))
    (save-excursion
      (goto-char (point-min))
      (when (re-search-forward "^#\\+GH_REPO:" nil t)
        (org-gh-mode 1)))))

;;;###autoload
(define-minor-mode org-gh-mode
  "Minor mode for syncing org headings with GitHub Issues.

\\{org-gh-mode-map}"
  :lighter " GH"
  :keymap org-gh-mode-map
  :group 'org-gh)

;;;###autoload
(add-hook 'org-mode-hook #'org-gh--maybe-enable)

(provide 'org-gh)
;;; org-gh.el ends here
