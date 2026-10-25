{
    flake.nixosModules.fonts = { pkgs, ... }:
    let
        dankMono = pkgs.runCommand "dank-mono" { } ''
            install -Dm444 -t $out/share/fonts/opentype ${./dank-mono}/*.otf
        '';
    in
    {
        fonts = {
            packages = [
                pkgs.font-manager
                
                dankMono
                pkgs.nerd-fonts.geist-mono
                pkgs.nerd-fonts.jetbrains-mono
                pkgs.nerd-fonts.iosevka
                pkgs.nerd-fonts.droid-sans-mono
                pkgs.material-symbols

                # Target Script Coverage
                pkgs.adwaita-fonts
                pkgs.lohit-fonts.devanagari
                pkgs.lohit-fonts.telugu
                pkgs.lohit-fonts.bengali
                pkgs.sarasa-gothic
                pkgs.font-arabic-misc
            ];

            fontconfig.defaultFonts = {
                monospace = [
                    "GeistMono Nerd Font"
                    "JetBrainsMono Nerd Font"
                    "Material Symbols"
                    "Iosevka Nerd Font"
                    "Noto Sans Mono CJK SC"
                    "Noto Sans Mono CJK JP"
                    "Noto Sans Mono CJK KR"
                    "Noto Sans Devanagari"
                ];

                sansSerif = [
                    "Noto Sans"
                    "Noto Sans CJK SC"
                    "Noto Sans CJK JP"
                    "Noto Sans CJK KR"
                    "Noto Sans Devanagari"
                ];
            };
        };
    };
}
