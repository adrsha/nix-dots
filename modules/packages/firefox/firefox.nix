{
    flake.nixosModules.firefox = { config, lib, pkgs, ... }:
    let
        chromeDir = pkgs.linkFarm "firefox-chrome" [
            { name = "userChrome.css";  path = ./userChrome.css; }
            { name = "userContent.css"; path = ./userContent.css; }
        ];
    in
    {
        programs.firefox = {
            enable = true;
            preferences = {
                # Required for userChrome.css / userContent.css to load.
                "toolkit.legacyUserProfileCustomizations.stylesheets" = true;
                # Stop GTK system colors bleeding into web pages. Best effort: pref may be ignored by newer versions.
                "widget.content.gtk-theme-override" = "Adwaita:dark";
                "layout.css.prefers-color-scheme.content-override" = 0;
            };
        };

        # Profile directory names are random, so link into every profile found.
        systemd.user.services.firefox-chrome = {
            wantedBy = [ "default.target" ];
            restartTriggers = [ chromeDir ];
            serviceConfig = { Type = "oneshot"; RemainAfterExit = true; };
            script = ''
                colors="$HOME/.config/theme-manager/firefox-colors.css"
                for dir in "$HOME"/.mozilla/firefox/*/ "$HOME"/.config/mozilla/firefox/*/; do
                    [ -f "$dir/prefs.js" ] || continue
                    # Replace the old single store symlink; leave a user-made real dir alone.
                    if [ -L "$dir/chrome" ]; then rm "$dir/chrome"; fi
                    mkdir -p "$dir/chrome"
                    ln -sfn ${chromeDir}/userChrome.css  "$dir/chrome/userChrome.css"
                    ln -sfn ${chromeDir}/userContent.css "$dir/chrome/userContent.css"
                    ln -sfn "$colors" "$dir/chrome/colors.css"
                done
            '';
        };
    };
}
