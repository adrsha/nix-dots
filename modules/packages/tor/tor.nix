{
    flake.nixosModules.tor = { pkgs, ... }: {
        environment.systemPackages = with pkgs; [ tor-browser ];
        services.tor = {
            enable = true;
            torsocks.enable = true;
            client.enable = true;

            relay = {
                enable = true;
                role = "relay";
            };

            settings = {
                ServerTransportListenAddr = [ "obfs4 0.0.0.0:8443" ];

                MaxAdvertisedBandwidth = "100 MB";
                BandwidthRate = "50 MB";
                RelayBandwidthRate = "50 MB";
                RelayBandwidthBurst = "100 MB";

                ExitNodes = "{is}";
                StrictNodes = true;

                CookieAuthentication = true;
                AvoidDiskWrites = 1;
                HardwareAccel = 1;
                SafeLogging = 1;
                NumCPUs = 8;

                ORPort = [ 443 ];
            };
        };

        services.snowflake-proxy = {
            enable = true;
            capacity = 10;
        };
    };
}
