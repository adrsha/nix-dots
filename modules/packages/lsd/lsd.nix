{
    flake.nixosModules.lsd = { config, lib, pkgs, ... }:
    let
        lsdConf = ./lsd;
        users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
    in
    {
        environment.systemPackages = [ pkgs.lsd ];

        systemd.tmpfiles.rules = lib.concatMap (u: [
			"d ${u.home}/.config/ - ${u.name} ${u.group} -"
            "L+ ${u.home}/.config/lsd - - - - ${lsdConf}"
        ]) users;

        systemd.user.services.systemd-tmpfiles-setup.restartTriggers = [
            lsdConf
        ];
    };
}
