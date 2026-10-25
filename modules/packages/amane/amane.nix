{ inputs, ... }:
{
    flake.nixosModules.amane = { config, lib, pkgs, ... }:
    let
        amaneSrc = ./amane/src;
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
    in
    {
        environment.systemPackages = [
            (inputs.amane.packages.${pkgs.stdenv.hostPlatform.system}.default.overrideAttrs (old: {
                patches = (old.patches or []) ++ [ ./amane/src/patches/ctrl-search-keys.patch ./amane/src/patches/scrolled-hitboxes.patch ./amane/src/patches/optimized-shell.patch ./amane/src/patches/pulse-drop-order.patch ];
            }))
            pkgs.ddcutil
            pkgs.libnotify
            pkgs.grim
            pkgs.slurp
            pkgs.wf-recorder
            pkgs.wl-clipboard
            pkgs.satty
            pkgs.swaylock
            pkgs.xdg-utils
            pkgs.python3
            # Wallpaper media support: ffmpeg decodes webp/gif/video thumbnails,
            # imagemagick falls back for webp/gif, mpvpaper plays video walls.
            pkgs.ffmpeg
            pkgs.imagemagick
            pkgs.mpvpaper
        ];

        hardware.i2c.enable = true;
        security.pam.services.swaylock = {};

        systemd.tmpfiles.rules = lib.concatMap (u: [
            "d ${u.home}/.config - ${u.name} ${u.group} -"
            "d ${u.home}/.config/amane - ${u.name} ${u.group} -"
            "L+ ${u.home}/.config/amane/src - - - - ${amaneSrc}"
        ]) users;

        systemd.user.services.systemd-tmpfiles-setup.restartTriggers = [ amaneSrc ];
    };
}
