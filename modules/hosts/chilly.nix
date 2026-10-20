{ inputs, config, ... }:
let
    nixosModules = config.flake.nixosModules;
in
{
    flake.nixosConfigurations.chilly = inputs.nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
            # Host-specific: things that only make sense for this machine.
            ../../nixos_config/hardware-configuration.nix
            {
                networking.hostName = "arch";
                # Never change after install, see `man configuration.nix`.
                system.stateVersion = "26.05";
            }

            # System
            nixosModules.boot
            nixosModules.session
            nixosModules.networking
            nixosModules.locale
            nixosModules.audio
            nixosModules.hardware
            nixosModules.users
            nixosModules.services
            nixosModules.packages
            nixosModules.nix

            # Programs / desktop
            nixosModules.mango
            nixosModules.fonts
            nixosModules.kitty
            nixosModules.nvim
            nixosModules.quickemu
            nixosModules.amane
            nixosModules.theme_manager
            nixosModules.firefox

            # Programs / terminal
            nixosModules.zsh
            nixosModules.lsd
        ];
    };
}
