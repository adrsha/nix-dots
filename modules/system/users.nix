{
    flake.nixosModules.users = { pkgs, ... }: {
        users.users.chilly = {
            isNormalUser = true;
            extraGroups = [ "wheel" "docker" ];
            shell = pkgs.zsh;
            home = "/home/chilly";
            packages = with pkgs; [ lsd fd fzf ripgrep igrep ];
        };

        security.sudo.extraRules = [{
            users = [ "chilly" ];
            commands = [{ command = "ALL"; options = [ "NOPASSWD" ]; }];
        }];
        
        virtualisation.docker.enable = true;
    };
}
