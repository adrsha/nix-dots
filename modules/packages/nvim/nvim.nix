# nvim.nix
#
# NixOS module that installs nvim and uses a plain git checkout of
# github.com/adrsha/nvim-perf as its config.
#
# On every `nixos-rebuild switch` (and on boot):
#     1. If <repoDir> is not a git repo yet, it is cloned there.
#     2. If it is, `git pull --ff-only` fetches new upstream commits.
#     3. ~/.config/nvim is a symlink to <repoDir>.
#
# <repoDir> is the folder `nvim-perf` next to this file:
# ~/flakes/modules/packages/nvim/nvim-perf. It cannot be the folder that holds
# this file, because that already contains nvim.nix and git will not clone into
# a non-empty folder. It is an ordinary folder, NOT in the read-only Nix store, so you can edit it
# freely (or `cd` in and use git). Your edits are never overwritten: a pull
# that would conflict with them just fails with a warning.
#
# Updating never breaks a rebuild. With no network (common at boot), or on any
# git error, the script prints a warning and the rebuild carries on.
#
# Notes:
#     - Add `modules/packages/nvim/nvim-perf/` to your flake's .gitignore,
#         because the checkout is a git repo inside your git repo.
#     - The checkout is owned by whoever owns its parent folder, so it stays
#         writable for you even though the rebuild runs as root.
#     - nvim-perf uses NvChad as a plugin, so on first start it downloads plugins
#         (network needed). Lazy writes lazy-lock.json into the checkout, which is
#         why the checkout must be writable.
#     - It configures LSP with vim.lsp.config, which needs nvim 0.11 or newer.
#     - It sets up language servers through Mason, and Mason's downloaded
#         binaries often do not run on NixOS. Install the servers you need via Nix
#         (for example in environment.systemPackages) instead.
{ ... }:
{
    flake.nixosModules.nvim = { config, lib, pkgs, ... }:
    let
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);

        # Absolute path of the checkout, next to this file. It cannot be derived
        # from `./.`: in a flake that is a store path. Nix also cannot read
        # $HOME, so the path is built from the first normal user's home.
        repoDir = "${(lib.head users).home}/flakes/modules/packages/nvim/nvim-perf";
        repoUrl = "https://github.com/adrsha/nvim-perf.git";

        updateConfig = pkgs.writeShellScript "update-nvim-config" ''
            set -eu
            export PATH=${lib.makeBinPath [ pkgs.git pkgs.util-linux pkgs.coreutils pkgs.getent ]}
            export GIT_TERMINAL_PROMPT=0 # fail instead of asking for credentials

            repo=${lib.escapeShellArg repoDir}
            parent=$(dirname "$repo")

            if [ ! -d "$parent" ]; then
                echo "nvim-config: $parent does not exist, check repoDir in nvim.nix" >&2
                exit 1
            fi

            # Run git as the folder's owner so the checkout stays writable for
            # that user (and git does not complain about foreign ownership).
            owner=$(stat -c %U "$parent")
            home=$(getent passwd "$owner" | cut -d: -f6)
            git_as_owner() {
                timeout 60 runuser -u "$owner" -- env HOME="$home" git "$@"
            }

            if [ -d "$repo/.git" ]; then
                git_as_owner -C "$repo" pull --ff-only
            else
                git_as_owner clone ${lib.escapeShellArg repoUrl} "$repo"
            fi
        '';
    in
    {
        assertions = [{
            assertion = users != [ ];
            message = "nvim.nix needs at least one normal user to locate the checkout.";
        }];

        # (The NixOS option is still called "neovim"; only the name is fixed.)
        programs.neovim.enable = true;

        # git: used here and by lazy.nvim. ripgrep: telescope's live_grep.
        # gcc: compiles treesitter parsers.
        environment.systemPackages = [ pkgs.git pkgs.ripgrep pkgs.gcc ];

        # Runs as root on every switch and boot. `||` keeps a failure from
        # aborting the rest of activation.
        system.activationScripts.updateNvimConfig = ''
            ${updateConfig} || echo "nvim-config: update failed, keeping the current checkout" >&2
        '';

        # Plain "L", not "L+": "L+" would delete an existing ~/.config/nvim
        # folder. If you already have one, move it away first.
        systemd.tmpfiles.rules = lib.concatMap (u: [
            "d ${u.home}/.config/ - ${u.name} ${u.group} -"
            "L ${u.home}/.config/nvim - - - - ${repoDir}"
        ]) users;
    };
}
