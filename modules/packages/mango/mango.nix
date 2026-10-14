{ inputs, ... }:
# NixOS module for the mangowc Wayland compositor.
# The mango config is written verbatim to ~/.config/mango/config.conf.
# Lines starting with # inside the config are comments for mango itself.
# Reference (latest): https://mangowm.github.io/docs
# After editing, check the syntax with `mango -p` (it prints unknown options).
{
	flake.nixosModules.mango = { config, lib, pkgs, ... }:
	let
		mangoConfig = pkgs.writeText "mango-config.conf" (builtins.readFile ./mango.conf);
		users = lib.filter (u: u.isNormalUser) (lib.attrValues config.users.users);
	in
	{
		# imports = [ inputs.mango.nixosModules.mango ];
		programs.mango.enable = true;

		# wlr-randr lists outputs, modes and refresh rates (needed to find monitor names).
		environment.systemPackages = [ pkgs.wlr-randr ];

		systemd.tmpfiles.rules = lib.concatMap (u: [
			"d ${u.home}/.config/ - ${u.name} ${u.group} -"
			"d ${u.home}/.config/mango - ${u.name} ${u.group} -"
			"L+ ${u.home}/.config/mango/config.conf - - - - ${mangoConfig}"
			]) users;

		systemd.user.services.systemd-tmpfiles-setup.restartTriggers = [ mangoConfig ];
	};
}
