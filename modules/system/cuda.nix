{
    flake.nixosModules.cuda = { pkgs, ... }:
    let
        # Pinned: the default (13.4) currently fails to build (patchelf crash).
        inherit (pkgs.cudaPackages_12) cudatoolkit cudnn;
    in
    {
        # Requires nixpkgs.config.allowUnfree (set in nix.nix).
        environment.systemPackages = [
            cudatoolkit 
            cudnn
            (pkgs.llama-cpp.override { cudaSupport = true; })
        ];

        environment.variables = {
            CUDA_PATH = "${cudatoolkit}";
            CUDA_HOME = "${cudatoolkit}";
        };

        # Prebuilt CUDA packages; `extra-` keeps cache.nixos.org active.
        nix.settings = {
            extra-substituters = [ "https://cuda-maintainers.cachix.org" ];
            extra-trusted-public-keys = [
                "cuda-maintainers.cachix.org-1:0dq3bujKpuEPMCX6U4WylrUDZ9JyUG0VpVZa7CNfq5E="
            ];
        };
    };
}
