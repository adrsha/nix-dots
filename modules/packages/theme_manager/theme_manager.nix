# theme_manager.nix
#
# Declarative NixOS module for github.com/adrsha/theme_manager.
# Importing it is all a host has to do; there is no enable option.
#
#     DECLARATIVE (this file, rebuilt by nixos-rebuild)
#         - the binaries, built by Nix from a pinned flake input
#         - the daemon, as a systemd user service
#         - the shell hooks (`theme` function, sourcing the generated env files)
#         - the themes directory link
#         - an optional default theme for a fresh machine
#         - the dconf/GSettings plumbing that color-scheme needs
#
#     RUNTIME STATE (written by theme-set / theme-daemon, never by Nix)
#         - ~/.local/state/theme/{current,amoled,transparent,active.json}
#         - the generated files in ~/.config (theme_vars.fish, theme_env.sh,
#             environment.d/theme-manager.conf, ghostty/kitty/gtk/qt/... files)
#
# flake.nix needs this input:
#     theme_manager = { url = "github:adrsha/theme_manager"; flake = false; };
{ inputs, ... }:
{
    flake.nixosModules.theme_manager = { config, lib, pkgs, ... }:
    let
        # ---- Settings: edit these here, not in the host file -------------
        # Theme applied only when none was ever selected (fresh install).
        # Later switches are never overridden. Set to null to disable.
        defaultTheme = "aquarium";

        # Directory with the theme files, linked to ~/.config/theme-manager/themes.
        # A Nix path (as below) is copied into the store: edits need a rebuild.
        # An absolute string such as
        #     "/home/<you>/flakes/modules/packages/theme_manager/themes"
        # links the live folder instead, so edits apply immediately.
        themesDir = "/home/chilly/flakes/modules/packages/theme_manager/themes";
        # ------------------------------------------------------------------

        src = inputs.theme_manager;
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
        user = (lib.head users).name;
        home = config.users.users.${user}.home;

        # --- dconf / GSettings plumbing -----------------------------------
        # Without the dconf GIO module, GLib silently falls back to an
        # in-memory backend: writes "succeed" but nothing is persisted, so the
        # portal never sees them. Every process that reads or writes
        # color-scheme must load this module.
        dconfModules = "${pkgs.dconf.lib}/lib/gio/modules";
        # Compiled schemas for org.gnome.desktop.interface (needed by gsettings).
        schemaDir = "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}";

        # Replays the last palette the daemon wrote onto every VT.
        ttyPalette = pkgs.writeShellScript "theme-tty-palette" ''
            f=${lib.escapeShellArg home}/.local/state/theme/tty_colors
            [ -r "$f" ] || exit 0
            for t in /dev/tty[1-6]; do
                { cat "$f" > "$t"; } 2>/dev/null
            done
        '';

        # Built by Nix instead of `cargo build` at activation time: reproducible,
        # cached, and no network or git needed at boot. Cargo.lock pins the crates.
        theme-manager = pkgs.rustPlatform.buildRustPackage {
            pname = "theme-manager";
            version = (lib.importTOML "${src}/Cargo.toml").package.version;
            inherit src;
            cargoLock.lockFile = "${src}/Cargo.lock";
            doCheck = false;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            # matugen: MATUGEN_SCHEME Material palettes.
            # ffmpeg: fallback MODE=dynamic palette extraction and video frames.
            # awww / mpvpaper: the daemon owns still and video wallpapers.
            # procps: pgrep/pkill for wallpaper process reconciliation.
            # dconf + glib: gsettings/dconf binaries, which are not on a bare
            #   NixOS PATH. Suffix keeps the system PATH first.
            # GIO_EXTRA_MODULES / XDG_DATA_DIRS: make GSettings persist to dconf
            #   instead of the in-memory fallback, whatever environment the
            #   binary is started from (terminal, systemd, keybind).
            postInstall = ''
                for b in theme-set theme-daemon; do
                    wrapProgram $out/bin/$b \
                        --suffix PATH : ${lib.makeBinPath [ pkgs.matugen pkgs.ffmpeg pkgs.awww pkgs.mpvpaper pkgs.procps pkgs.dconf pkgs.glib ]} \
                        --set GIO_EXTRA_MODULES ${dconfModules} \
                        --suffix XDG_DATA_DIRS : ${schemaDir}
                done
            '';
            meta.mainProgram = "theme-set";
        };

        # Same wiring the README asks you to paste into rc files, declared once.
        # Every new shell already gets the vars from the generated files; the
        # `theme` wrapper additionally re-sources them in the *current* shell.
        posixInit = ''
            theme() {
                command theme-set "$@" || return
                if [ -f "$HOME/.config/theme-manager/theme_env.sh" ]; then
                    . "$HOME/.config/theme-manager/theme_env.sh"
                fi
            }
            if [ -f "$HOME/.config/theme-manager/theme_env.sh" ]; then
                . "$HOME/.config/theme-manager/theme_env.sh"
            fi
        '';

        # Runs before the daemon starts, as the user.
        prep = pkgs.writeShellScript "theme-manager-prep" ''
            set -eu
            export PATH=${lib.makeBinPath [ pkgs.coreutils ]}:$PATH

            mkdir -p "$HOME/.local/state/theme" "$HOME/.config/theme-manager"

            # Point ~/.config/theme-manager/themes at the declared directory.
            # A real directory (e.g. left over from the Arch setup) is never touched.
            link="$HOME/.config/theme-manager/themes"
            if [ -L "$link" ] || [ ! -e "$link" ]; then
                ln -sfn ${lib.escapeShellArg (toString themesDir)} "$link"
            else
                echo "theme-manager: $link is a real directory, leaving it alone" >&2
            fi

            ${lib.optionalString (defaultTheme != null) ''
                # First boot only: pick a theme if none was ever selected.
                if [ ! -e "$HOME/.local/state/theme/current" ]; then
                    ${theme-manager}/bin/theme-set ${lib.escapeShellArg defaultTheme} || true
                fi
            ''}
        '';
    in
    {
        assertions = [{
            assertion = users != [ ];
            message = "theme_manager.nix needs at least one normal user.";
        }];

        environment.systemPackages = [
            theme-manager
            pkgs.matugen
            pkgs.awww
            pkgs.mpvpaper
            # Handy for debugging and manual runs: `dconf read/watch`, `gsettings`.
            pkgs.dconf
            pkgs.glib
        ];

        # theme-set pushes the color scheme through dconf.
        programs.dconf = {
            enable = lib.mkDefault true;
            profiles.user.databases = [{
                settings."org/gnome/desktop/interface" = {
                    gtk-theme = "ThemeManager";
                    color-scheme = "prefer-dark";
                };
            }];
        };

        # Portal: the thing Firefox actually asks for prefers-color-scheme.
        # The portal and its gtk backend must load the dconf GIO module too,
        # or they read an empty in-memory store and report 0 ("no preference").
        xdg.portal = {
            enable = true;
            extraPortals = [ pkgs.xdg-desktop-portal-gtk ];
            config.common.default = lib.mkDefault [ "gtk" ];
        };
        systemd.user.services.xdg-desktop-portal.environment.GIO_EXTRA_MODULES = dconfModules;
        systemd.user.services.xdg-desktop-portal-gtk.environment.GIO_EXTRA_MODULES = dconfModules;

        # Shell hooks. Harmless for shells you don't use.
        # (fish auto-sources ~/.config/fish/conf.d/theme_vars.fish by itself.)
        programs.fish.interactiveShellInit = ''
            function theme
                theme-set $argv
                and source ~/.config/fish/conf.d/theme_vars.fish
            end
        '';
        programs.bash.interactiveShellInit = posixInit;
        programs.zsh.interactiveShellInit = posixInit;

        # Repaint the Linux console at login. The daemon handles live theme changes,
        # but a fresh TTY login after boot starts with the build-time colors.
        environment.interactiveShellInit = ''
            if [ "$TERM" = linux ]; then
                cat "$HOME/.local/state/theme/tty_colors" 2>/dev/null >/dev/tty
            fi
        '';

        # Paint the console at boot, before any login prompt is used. Without this,
        # new TTYs start with the build-time console.colors until a theme is applied.
        systemd.services.theme-tty = {
            description = "Apply theme-manager palette to the Linux console";
            wantedBy = [ "multi-user.target" ];
            after = [ "systemd-vconsole-setup.service" "local-fs.target" ];
            serviceConfig = {
                Type = "oneshot";
                ExecStart = ttyPalette;
            };
        };

        # The daemon watches ~/.local/state/theme/current (inotify) and
        # regenerates every config when it changes. This replaces the
        # `hl.exec_cmd("theme-daemon")` autostart line.
        systemd.user.services.theme-daemon = {
            description = "theme-manager daemon";
            wantedBy = [ "graphical-session.target" ];
            partOf = [ "graphical-session.target" ];
            after = [ "graphical-session.target" ];
            # Writers only run for apps found on PATH (or with an existing
            # config dir), and reloads call hyprctl, systemctl, swaync-client...
            path = [
                "/run/wrappers"
                "/run/current-system/sw"
                "/etc/profiles/per-user/${user}"
            ];
            serviceConfig = {
                ExecStartPre = "-${prep}";
                ExecStart = "${theme-manager}/bin/theme-daemon";
                Restart = "on-failure";
                RestartSec = 2;
            };
        };
    };
}
