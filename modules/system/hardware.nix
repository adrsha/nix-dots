{
    flake.nixosModules.hardware = { config, ... }: {
        hardware.graphics.enable = true;
        hardware.bluetooth.enable = true;
        services.libinput.enable = false;

        # NVIDIA (requires nixpkgs.config.allowUnfree, set in nix.nix)
        services.xserver.videoDrivers = [ "nvidia" ];
        hardware.nvidia = {
            modesetting.enable = true;
            open = true;
            package = config.boot.kernelPackages.nvidiaPackages.stable;
        };
    };
}
