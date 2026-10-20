{
    flake.nixosModules.console = { pkgs, ... }: {
        console = {
            earlySetup = true;
            font = "${pkgs.terminus_font}/share/consolefonts/LatGrkCyr-8x16.psfu.gz";
            packages = with pkgs; [ terminus_font ];
            useXkbConfig = true;
            # colors handled by theme_manager
        };
    };
}
