{
    flake.nixosModules.session = {
        # Needs a re-login to take effect.
        environment.sessionVariables = {
            EDITOR = "nvim";
            VISUAL = "nvim";
            BROWSER = "firefox";
            TERMINAL = "kitty";
            MOZ_ENABLE_WAYLAND = "1";
            QT_QPA_PLATFORM = "wayland";
            XCURSOR_SIZE = "24";
            NIXOS_OZONE_WL = "1";

            # NVIDIA + wlroots
            WLR_NO_HARDWARE_CURSORS = "1";
            LIBVA_DRIVER_NAME = "nvidia";
            __GLX_VENDOR_LIBRARY_NAME = "nvidia";
            GBM_BACKEND = "nvidia-drm";
            __GL_GSYNC_ALLOWED = "1";
            __GL_VRR_ALLOWED = "1";
        };
    };
}
