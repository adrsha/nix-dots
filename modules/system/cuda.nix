{
    flake.nixosModules.cuda = { pkgs, ... }:
    {
        # Requires nixpkgs.config.allowUnfree (set in nix.nix).
        environment.systemPackages = [
            (pkgs.llama-cpp.override { cudaSupport = true; })
        ];

        nix.settings = {
            extra-substituters = [ "https://nix-community.cachix.org" ];
            extra-trusted-public-keys = [
                "nix-community.cachix.org-1:mB9FSh9qf2dCimDSUo8Zy7bkq5CX+/rkCWyvRCYg3Fs="
            ];
        };
    };
}
