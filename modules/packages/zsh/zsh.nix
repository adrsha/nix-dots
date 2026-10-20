{
    flake.nixosModules.zsh = { config, lib, pkgs, ... }:
    let
        zshConfig = ./zshrc.zsh;
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
    in
    {
        programs.zsh = {
            enable = true;
            enableCompletion = false;
            enableGlobalCompInit = false;
            promptInit = "";
            interactiveShellInit = builtins.readFile zshConfig;
        };
        programs.starship = {
            enable = true;
            settings = {
                add_newline = false;
                command_timeout = 500;
                scan_timeout = 30;

                format = lib.concatStrings [
                    "$username"
                    "$hostname"
                    "$directory"
                    "$git_branch"
                    "$git_commit"
                    "$git_state"
                    "$git_status"
                    "$nix_shell"
                    "$nodejs"
                    "$golang"
                    "$rust"
                    "$php"
                    "$lua"
                    "$cmd_duration"
                    "$fill"
                    "$time"
                    "$line_break"
                    "$character"
                ];


                character = {
                    success_symbol = "[❯](bold green)";
                    error_symbol = "[❯](bold red)";
                };

                directory = {
                    truncation_length = 3;
                    truncate_to_repo = true;
                    read_only = " 󰌾";
                };

                cmd_duration.min_time = 2000;

                git_branch.symbol = " ";
                nix_shell.symbol = " ";
                nodejs.symbol = " ";
                golang.symbol = " ";
                rust.symbol = " ";
                php.symbol = " ";
                lua.symbol = " ";
            };
        };

        environment.systemPackages = with pkgs; [ zoxide bat ];

        systemd.tmpfiles.rules = lib.concatMap (u: [
                "L+ ${u.home}/.zshrc - - - - ${zshConfig}"
        ]) users;

        systemd.user.services.systemd-tmpfiles-setup.restartTriggers = [
            zshConfig
        ];
    };
}
