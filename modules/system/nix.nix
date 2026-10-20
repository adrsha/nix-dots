{
    flake.nixosModules.nix = {
        nix.settings.experimental-features = [ "nix-command" "flakes" ];
        nixpkgs.config.allowUnfree = true;
        # Not supported with flakes.
        system.copySystemConfiguration = false;
    };
}
