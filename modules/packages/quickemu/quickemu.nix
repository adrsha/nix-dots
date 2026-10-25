{ inputs, ... }:

{
  flake.nixosModules.quickemu = { config, lib, pkgs, ... }:
    {
      # nixpkgs.overlays = [
      #   inputs.quickemu.overlays.default
      # ];
      #
      # environment.systemPackages = [
      #   pkgs.quickemu
      # ];
    };
}
