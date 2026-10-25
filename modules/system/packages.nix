{
    flake.nixosModules.packages = { pkgs, ... }: {
        environment.systemPackages = with pkgs; [
                neovim
                wget
                curl
                git
                rustup
                python3
                unzip
                zip
                file
                killall
                nix-sweep

                tree-sitter
                vscode-langservers-extracted
                clang-tools
                bash-language-server
                lua-language-server
                nil
                dart
                rust-analyzer
                ruff
                shfmt
                kotlin-language-server
                eslint_d
                astro-language-server
                typescript
                typescript-language-server
                pyright
                nixfmt

                qbittorrent
                mpv
                btop
                opencode
                wev
                bubblewrap
                bibata-cursors

                caido-desktop
                ungoogled-chromium
                appimage-run
                steam-run

                llama-cpp-cuda
        ];
    };
}
