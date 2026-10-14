{ inputs, config, ...}:
{
    flake.nixosConfigurations.chilly = inputs.nixpkgs.lib.nixosSystem {
        system = "x86_64-linux";
        modules = [
            ../../nixos_config/configuration.nix 
            config.flake.nixosModules.mango
            config.flake.nixosModules.fonts
            config.flake.nixosModules.kitty
            config.flake.nixosModules.nvim
            config.flake.nixosModules.theme_manager
        ];
    };
}
