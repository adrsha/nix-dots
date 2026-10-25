{
    flake.nixosModules.desktop = { pkgs, ... }: {
        xdg.portal = {
            enable = true;
            wlr.enable = true;                          # screen capture backend for wlroots compositors
            extraPortals = [ pkgs.xdg-desktop-portal-gtk ]; # file pickers, etc.

            # `common` is the fallback for any XDG_CURRENT_DESKTOP value,
            # so this works regardless of what mango reports.
            config.common = {
                default = [ "gtk" ];
                "org.freedesktop.impl.portal.ScreenCast" = [ "wlr" ];
                "org.freedesktop.impl.portal.Screenshot" = [ "wlr" ];
            };
        };

        # Screen capture streams go through PipeWire. [pipewire.nix]
        security.rtkit.enable = true;
        security.polkit.enable = true;

        # Source picker: click on the monitor you want to share.
        environment.etc."xdg/xdg-desktop-portal-wlr/config".text = ''
            [screencast]
            chooser_type=simple
            chooser_cmd=${pkgs.slurp}/bin/slurp -f %o -or
        '';

        # Makes Electron/Chromium apps (Discord, browsers) run natively on Wayland. [sessions.nix] var: NIXOS_OZONE
    };
}
