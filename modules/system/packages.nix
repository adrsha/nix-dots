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
                pyright
                nixfmt

                qbittorrent
                mpv
                btop
                opencode
                wev
                bubblewrap
        ];
    };
}
