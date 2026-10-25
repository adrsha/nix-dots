{
    flake.nixosModules.emacs = { config, lib, pkgs, ... }:
    let
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);

        cursorTheme = "Bibata-Modern-Classic";
        cursorSize = "24";
        cursorPaths = [
            "${pkgs.bibata-cursors}/share/icons"
            "${pkgs.adwaita-icon-theme}/share/icons"
        ];

        # which-key and use-package are built into Emacs 30+, so they are not listed.
        # SVG images (svg-tag-mode) need Emacs built with librsvg. nixpkgs adds librsvg
        # to the pgtk and X builds automatically and there is no switch for it, so plain
        # emacs-pgtk is enough. emacs-nox (used below only to tangle) has no SVG support.
        # Check the running daemon with:
        #   emacsclient -e "(image-type-available-p 'svg)"
        emacs = pkgs.emacs-pgtk.pkgs.withPackages (epkgs:
        let
            # Not on MELPA or in nixpkgs, so build it from GitHub, pinned to a commit.
            # If Nix reports a hash mismatch, paste the hash it prints into `hash`.
            # This is a plain derivation instead of melpaBuild: melpaBuild failed inside
            # package.el's install step. The files are byte-compiled when that works and
            # installed as plain .el otherwise, and dependencies are propagated so the
            # Emacs wrapper puts them on the load path.
            punch-line = pkgs.stdenvNoCC.mkDerivation {
                pname = "punch-line";
                version = "0-unstable-2026-02-25";
                src = pkgs.fetchFromGitHub {
                    owner = "konrad1977";
                    repo = "punch-line";
                    rev = "1b592e0bef5678dcb7bf78557b43a308321985c0";
                    hash = "sha256-G9MSuc+X2EcnPkXimUC420efxT8Jd3eDTP9ik1zNhvo=";
                };
                # punch-line requires flycheck at load time and async in its header.
                propagatedBuildInputs = with epkgs; [ nerd-icons flycheck async ];
                nativeBuildInputs = [
                    (pkgs.emacs-nox.pkgs.withPackages (e: with e; [ nerd-icons flycheck async ]))
                ];
                buildPhase = ''
                    runHook preBuild
                    export HOME="$TMPDIR"
                    emacs --batch -Q -L . -f batch-byte-compile *.el || true
                    runHook postBuild
                '';
                installPhase = ''
                    runHook preInstall
                    mkdir -p "$out/share/emacs/site-lisp"
                    cp *.el "$out/share/emacs/site-lisp/"
                    cp *.elc "$out/share/emacs/site-lisp/" 2>/dev/null || true
                    runHook postInstall
                '';
            };
        in
        with epkgs; [
            punch-line
            flycheck # required by punch-line, never enabled (diagnostics use Flymake)
            async # required by punch-line; listed here so the Emacs wrapper puts it on the load path
            general
            evil
            evil-collection
            evil-commentary
            evil-escape
            evil-surround
            evil-textobj-anyblock
            evil-org
            undo-fu
            undo-fu-session
            helpful
            orderless
            devdocs
            openwith
            company
            org-modern
            visual-fill-column
            org-appear
            svg-tag-mode # pulls in svg-lib; needs librsvg in Emacs itself
            org-roam
            org-roam-ui
            popper
            gcmh
            vertico
            consult
            embark
            embark-consult
            nerd-icons
            nerd-icons-completion
            nerd-icons-dired
            iedit
            smartparens
            yasnippet
            yasnippet-snippets
            lsp-mode
            lsp-ui
            dart-mode
            flutter
            lsp-dart
            format-all
            rainbow-mode
            rainbow-delimiters
            drag-stuff
            auctex
            avy
            treesit-grammars.with-all-grammars
        ]);

        syntaxCheck = pkgs.writeText "check-syntax.el" ''
            (dolist (file '("early-init.el" "init.el"))
              (with-temp-buffer
                (insert-file-contents file)
                (emacs-lisp-mode)
                (goto-char (point-min))
                (unless (re-search-forward "lexical-binding: *t" (line-end-position) t)
                  (message "%s: missing lexical-binding cookie" file)
                  (kill-emacs 1))
                (goto-char (point-min))
                (condition-case err
                    (while (progn (forward-comment (buffer-size)) (not (eobp)))
                      (read (current-buffer)))
                  (error
                   (message "%s: %S (near position %d)" file err (point))
                   (kill-emacs 1)))))
        '';

        emacsConfig = pkgs.runCommand "emacs-config" { nativeBuildInputs = [ pkgs.emacs-nox ]; } ''
            export HOME="$TMPDIR"
            cp --no-preserve=mode ${./init.org} init.org
            emacs --batch -Q --eval "(require 'org)" --eval '(org-babel-tangle-file "init.org")'
            test -s early-init.el
            test -s init.el
            emacs --batch -Q --load ${syntaxCheck}
            mkdir -p "$out"
            install -m444 early-init.el init.el init.org -t "$out"
        '';
    in
    {
        environment.systemPackages = [ emacs ] ++ (with pkgs; [
            ripgrep
            fd
            pandoc
            texliveSmall
            graphviz
            xdg-utils
            shfmt
            astyle
            bibata-cursors
            adwaita-icon-theme
        ]);

        environment.sessionVariables = {
            XCURSOR_THEME = cursorTheme;
            XCURSOR_SIZE = cursorSize;
            XCURSOR_PATH = cursorPaths;
        };

        fonts.packages = with pkgs; [
            nerd-fonts.jetbrains-mono
            # init.org sets "Iosevka Nerd Font" as the default face. The SVG tags measure
            # their text with that exact family and fail if it is missing.
            nerd-fonts.iosevka
            # nerd-icons (punch-line, dired, completion) draws its glyphs from this family.
            nerd-fonts.symbols-only
            adwaita-fonts
        ];

        services.emacs = {
            enable = true;
            package = emacs;
            defaultEditor = true;
        };

        systemd.user.services.emacs = {
            environment = {
                XCURSOR_THEME = cursorTheme;
                XCURSOR_SIZE = cursorSize;
                XCURSOR_PATH = lib.concatStringsSep ":" cursorPaths;
            };
            restartTriggers = [ emacsConfig ];
        };

        systemd.tmpfiles.rules = lib.concatMap (u: [
            "d ${u.home}/.config - ${u.name} ${u.group} -"
            "d ${u.home}/.config/emacs - ${u.name} ${u.group} -"
            "L+ ${u.home}/.config/emacs/early-init.el - - - - ${emacsConfig}/early-init.el"
            "L+ ${u.home}/.config/emacs/init.el - - - - ${emacsConfig}/init.el"
            "L+ ${u.home}/.config/emacs/init.org - - - - ${emacsConfig}/init.org"
        ]) users;
    };
}
