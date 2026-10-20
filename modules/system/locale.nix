{
    flake.nixosModules.locale = {
        time.timeZone = "Asia/Kathmandu";
        i18n.defaultLocale = "en_US.UTF-8";
    };
}
