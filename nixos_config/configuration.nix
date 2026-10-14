# Edit this configuration file to define what should be installed on
# your system. Help is available in the configuration.nix(5) man page, on
# https://search.nixos.org/options and in the NixOS manual (`nixos-help`).

{ config, lib, pkgs, ... }:

{
    imports =
        [ # Include the results of the hardware scan.
            ./hardware-configuration.nix
        ];

    # Use the systemd-boot EFI boot loader.
    boot.loader.systemd-boot.enable = true;
    boot.loader.efi.canTouchEfiVariables = true;

    networking.hostName = "arch"; # Define your hostname.

    # Configure network connections interactively with nmcli or nmtui.
    #networking.networkmanager.enable = true;
    systemd.network.enable = true;
    systemd.network.networks."10-wan" = {
        matchConfig.Name = "enp12s0";
        networkConfig = {
        DHCP = "ipv4";
        IPv6AcceptRA = true;
        # DNS = ["1.1.1.1" "8.8.8.8"];
    };
    dhcpV4Config.UseDNS = false;
    dhcpV6Config.UseDNS = false;
    linkConfig.RequiredForOnline = "routable";
    };
    networking.nameservers = ["1.1.1.1" "8.8.8.8"];
    services.resolved = {
        enable = true;
        dnssec = "true";
        domains = [ "~." ];
        fallbackDns = ["1.1.1.1" "1.0.0.1" "8.8.8.8" "8.8.4.4"];
        dnsovertls = "true";
    };

    # Set your time zone.
    time.timeZone = "Asia/Kathmandu";

    # Select internationalisation properties.
    i18n.defaultLocale = "en_US.UTF-8";
    console = {
        font = "Lat2-Terminus16";
        useXkbConfig = true; # use xkb.options in tty.
    };

    # Enable sound.
    services.pipewire = {
        enable = true;
        pulse.enable = true;
    };

    # Enable touchpad support (enabled default in most desktopManager).
    services.libinput.enable = false;

    # Define a user account. Don't forget to set a password with ‘passwd’.
    users.users.chilly = {
         isNormalUser = true;
         extraGroups = [ "wheel" ]; # Enable ‘sudo’ for the user.
         packages = with pkgs; [
            lsd
            fd
            fzf
            ripgrep
            igrep
            firefox
         ];
         shell = pkgs.bash;
         home = "/home/chilly";
    };

    programs.kdeconnect.enable = true;

    # List packages installed in system profile.
    # You can use https://search.nixos.org/ to find more packages (and options).
    environment.systemPackages = with pkgs; [
        neovim # Do not forget to add an editor to edit configuration.nix! The Nano editor is also installed by default.
        wget
        curl
        git
    ];

    # Some programs need SUID wrappers, can be configured further or are
    # started in user sessions.
    # programs.mtr.enable = true;
    # programs.gnupg.agent = {
    #     enable = true;
    #     enableSSHSupport = true;
    # };

    # Enable the OpenSSH daemon.
    services.openssh.enable = true;

    # Open ports in the firewall.
    networking.firewall = {
        enable = true;
        allowedTCPPortRanges = [ { from = 1714; to = 1764; } ];
        allowedUDPPortRanges = [ { from = 1714; to = 1764; } ];
    };
    hardware.bluetooth.enable = true; 

    nixpkgs.config.allowUnfree = true;
    hardware.graphics.enable = true;
    services.xserver.videoDrivers = [ "nvidia" ];
    hardware.nvidia = {
        modesetting.enable = true;
        open = true;
        package = config.boot.kernelPackages.nvidiaPackages.stable;
    };

    nix.settings.experimental-features = ["nix-command" "flakes"];

    security.sudo.extraRules = [
        {
            users = ["chilly"];
            commands = [
                {
                    command = "ALL";
                    options = [ "NOPASSWD" ];
                }
            ];
        }
    ];

    # Copy the NixOS configuration file and link it from the resulting system
    # (/run/current-system/configuration.nix). This is useful in case you
    # accidentally delete configuration.nix.
    system.copySystemConfiguration = false; # As flakes as used and they dont support this.

    # This option defines the first version of NixOS you have installed on this particular machine,
    # and is used to maintain compatibility with application data (e.g. databases) created on older NixOS versions.
    #
    # Most users should NEVER change this value after the initial install, for any reason,
    # even if you've upgraded your system to a new NixOS release.
    #
    # This value does NOT affect the Nixpkgs version your packages and OS are pulled from,
    # so changing it will NOT upgrade your system - see https://nixos.org/manual/nixos/stable/#sec-upgrading for how
    # to actually do that.
    #
    # This value being lower than the current NixOS release does NOT mean your system is
    # out of date, out of support, or vulnerable.
    #
    # Do NOT change this value unless you have manually inspected all the changes it would make to your configuration,
    # and migrated your data accordingly.
    #
    # For more information, see `man configuration.nix` or https://nixos.org/manual/nixos/stable/options#opt-system.stateVersion .
    system.stateVersion = "26.05"; # Did you read the comment?

}

