{
    flake.nixosModules.mango = { config, lib, pkgs, ... }:
    let
        mangoConfig = ./mango.conf;
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
    in
    {
        # imports = [ inputs.mango.nixosModules.mango ];
        programs.mango.enable = true;

        environment.systemPackages = with pkgs; [ wlr-randr xdg-desktop-portal-wlr wl-clipboard ];

        systemd.tmpfiles.rules = lib.concatMap (u: [
            "d ${u.home}/.config/ - ${u.name} ${u.group} -"
            "d ${u.home}/.config/mango - ${u.name} ${u.group} -"
            "L+ ${u.home}/.config/mango/config.conf - - - - ${mangoConfig}"
            ]) users;

        systemd.user.services.systemd-tmpfiles-setup.restartTriggers = [ mangoConfig ];
    };
}
