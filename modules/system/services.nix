{
    flake.nixosModules.services = {
        services.openssh.enable = true;
        programs.kdeconnect.enable = true;
    };
}
