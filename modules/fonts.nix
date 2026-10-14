# fonts.nix
#
# NixOS module that installs system fonts and sets the default monospace font.
#
# Dank Mono is a commercial font, so it is NOT in nixpkgs and its files cannot
# be committed to a public repo. Setup:
#   1. Buy it at https://dank.sh and unzip it.
#   2. Copy the .otf files (Regular, Italic, Bold, ...) into ./dank-mono/
#      (the folder next to this file).
#   3. `git add` them. Flakes only see git-tracked files, so an untracked font
#      fails with "path does not exist". Keep the repo private.
#
# If the font installs but kitty ignores it, the font is probably not flagged
# as monospaced (reported for kitty >= 0.36). Check with:
#   fc-list :spacing=mono | grep -i dank
{ inputs, ... }:
{
	flake.nixosModules.fonts = { pkgs, ... }:
	let
		# Copies ./dank-mono/*.otf into the store where fontconfig looks for fonts.
		# The build fails loudly if the folder has no .otf files.
		# dankMono = pkgs.runCommand "dank-mono" { } ''
		# 	install -Dm444 -t $out/share/fonts/opentype ${./dank-mono}/*.otf
		# '';
	in
	{
		fonts = {
			packages = [
				# dankMono
				# Fallback for icon glyphs (Nerd Font symbols) that Dank Mono lacks.
				pkgs.nerd-fonts.jetbrains-mono
			];
			fontconfig.defaultFonts = {
				# First entry wins; later ones fill in missing glyphs.
				monospace = [ "JetBrainsMono Nerd Font" ];
			};
		};
	};
}
