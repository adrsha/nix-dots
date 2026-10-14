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
#
#     RUNTIME STATE (written by theme-set / theme-daemon, never by Nix)
#         - ~/.local/state/theme/{current,amoled,transparent,active.json}
#         - the generated files in ~/.config (theme_vars.fish, theme_env.sh,
#             environment.d/theme-manager.conf, ghostty/kitty/gtk/qt/... files)
#
# Your own app configs stay declarative and just *include* the generated
# files. Switching themes rewrites those files and reloads the apps; Nix is
# not involved and nothing needs a rebuild.
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
        themesDir = ./themes;
        # ------------------------------------------------------------------

        src = inputs.theme_manager;
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
        user = (lib.head users).name;

        # Built by Nix instead of `cargo build` at activation time: reproducible,
        # cached, and no network or git needed at boot. Cargo.lock pins the crates.
        theme-manager = pkgs.rustPlatform.buildRustPackage {
            pname = "theme-manager";
            version = (lib.importTOML "${src}/Cargo.toml").package.version;
            inherit src;
            cargoLock.lockFile = "${src}/Cargo.lock";
            doCheck = false;
            nativeBuildInputs = [ pkgs.makeWrapper ];
            # ffmpeg is needed for MODE=dynamic (palette from the wallpaper).
            # --suffix keeps the system PATH first, so hyprctl, swaync-client,
            # systemctl and friends are still found where they normally are.
            postInstall = ''
                for b in theme-set theme-daemon; do
                    wrapProgram $out/bin/$b --suffix PATH : ${lib.makeBinPath [ pkgs.ffmpeg ]}
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

        environment.systemPackages = [ theme-manager ];

        # theme-set pushes the color scheme through gsettings/dconf.
        programs.dconf.enable = lib.mkDefault true;

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
