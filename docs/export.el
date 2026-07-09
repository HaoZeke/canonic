;; Setup Package Manager (to fetch ox-rst automatically)
(require 'package)
(add-to-list 'package-archives '("melpa" . "https://melpa.org/packages/") t)
(add-to-list 'package-archives '("gnu" . "https://elpa.gnu.org/packages/") t)
(package-initialize)

;; Ubuntu emacs-nox ships an older Org that lacks helpers MELPA ox-rst needs
;; (e.g. org-element-type-p). Install current Org from GNU ELPA first.
(unless (package-installed-p 'org)
  (package-refresh-contents)
  (package-install 'org))
;; Prefer ELPA Org over the bundled one.
(require 'org)

;; Ensure ox-rst is present
(unless (package-installed-p 'ox-rst)
  (package-refresh-contents)
  (package-install 'ox-rst))

(require 'ox-rst)
(require 'ox-publish)

;; Enable org-babel evaluation for dot (graphviz) blocks
(require 'ob-dot)
(setq org-confirm-babel-evaluate nil)

;; Define the Publishing Project
(setq org-publish-project-alist
      '(("sphinx-rst"
         :base-directory "./orgmode/"
         :base-extension "org"
         :publishing-directory "./source/"
         :publishing-function org-rst-publish-to-rst
         :recursive t
         :headline-levels 4)
        ("sphinx-images"
         :base-directory "./orgmode/"
         :base-extension "svg\\|png\\|jpg"
         :publishing-directory "./source/"
         :publishing-function org-publish-attachment
         :recursive t)
        ("sphinx" :components ("sphinx-rst" "sphinx-images"))))

;; Run the publish
(org-publish "sphinx" t)
