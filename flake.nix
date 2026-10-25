{
    description = "A top level flake";

    inputs = {
        nixpkgs.url     = "github:NixOS/nixpkgs/nixpkgs-unstable";
        flake-parts.url = "github:hercules-ci/flake-parts";
        import-tree.url = "github:vic/import-tree";

        mango.url = "github:mangowm/mango";
        theme_manager = { url = "github:adrsha/theme_manager"; flake = false; };
        amane.url = "github:MystiaFin/amane";
        quickemu.url = "github:quickemu-project/quickemu";
    };

    outputs = inputs:
        inputs.flake-parts.lib.mkFlake { inherit inputs; }
        (inputs.import-tree ./modules);
}
