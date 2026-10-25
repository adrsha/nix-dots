{
    flake.nixosModules.networking = {
        networking.nameservers = [ "1.1.1.1" "8.8.8.8" ];

        # networkd is the only network manager; the global DHCP client must be off.
        # networking.useNetworkd = true;
        # networking.useDHCP = false;
        systemd.network.enable = true;

        systemd.network.networks."10-wan" = {
            matchConfig.Name = "enp12s0";
            networkConfig = {
                DHCP = "ipv4";
                IPv6AcceptRA = true;
            };
            dhcpV4Config.UseDNS = false;
            dhcpV6Config.UseDNS = false;
            linkConfig.RequiredForOnline = "routable";
        };

        services.resolved = {
            enable = true;
            settings.Resolve = {
                DNSSEC = "true";
                DNSOverTLS = "true";
                Domains = [ "~." ];
                FallbackDNS = [ "1.1.1.1" "1.0.0.1" "8.8.8.8" "8.8.4.4" ];
            };
        };

        # services.openvpn.servers.work = {
        #     autoStart = false;
        #     config = "config ~/flakes/modules/vpns/Iceland-TUN.ovpn";
        # };

        # KDE Connect ports (1714-1764) are opened by programs.kdeconnect.enable.
        networking.firewall.enable = true;
    };
}
