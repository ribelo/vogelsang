{
  description = "A flake for your project";

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

          native_libs = with pkgs; [
            gfortran
            pkg-config
            openssl
          ];
        in
        with pkgs;
        {
          devShells.default = mkShell {
            nativeBuildInputs = native_libs;
            shellHook = ''
              export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${pkgs.lib.makeLibraryPath native_libs}:./"
            '';
          };
        });
}
