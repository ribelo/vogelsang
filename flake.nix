{
  description = "A flake for vogelsang project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem
      (system:
        let
          system = "x86_64-linux";
          pkgs = import nixpkgs {
            inherit system;
          };
          native-libs = with pkgs; [ cmake pkg-config wrapGAppsHook ];

          libs = with pkgs; [
            libxkbcommon
            libGL
            openssl

            # WINIT_UNIX_BACKEND=wayland
            wayland

            # WINIT_UNIX_BACKEND=x11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            xorg.libX11
            # File Picker
            fontconfig
            gdk-pixbuf
            cairo
            pango
            gtk3
            gsettings-desktop-schemas
          ];
        in
        with pkgs;
        {
          devShells.default = mkShell {
            nativeBuildInputs = native-libs;
            buildInputs = libs;
            shellHook = ''
              export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath libs}:./"
              export XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS
            '';
          };
        });
}
