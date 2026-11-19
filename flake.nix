{
  description = "A static verifier for Rust, based on the Viper verification infrastructure.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk.url = "github:nmattia/naersk";
    rust-overlay.url = "github:oxalica/rust-overlay";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, naersk, rust-overlay, utils }:
    utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlay ];
        };

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain;

        naersk-lib = naersk.lib."${system}".override {
          cargo = rust;
          rustc = rust;
        };

        prusti-version = "${self.tag or "${self.lastModifiedDate}.${self.shortRev or "dirty"}"}";
      in rec {
        packages = {
          prusti = naersk-lib.buildPackage {
            name = "prusti";
            version = "${prusti-version}";
            root = ./.;
            buildInputs = [
              pkgs.pkg-config
              pkgs.wget
              pkgs.gcc
              pkgs.openssl
              pkgs.jdk11
              packages.viper
            ];
            nativeBuildInputs = [
              pkgs.makeWrapper
              pkgs.python3
            ];

            copyTarget = true;
            compressTarget = false;

            override = _: {
              preBuild = ''
                export LD_LIBRARY_PATH="${pkgs.jdk11}/lib/openjdk/lib/server"
                export VIPER_HOME="${packages.viper}/backends"
                export Z3_EXE="${packages.viper}/z3/bin/z3"
              '';
            };
            overrideMain = _: {
              postInstall = ''
                # `x.py package` copies the binaries, the contracts and the source tree's
                # `viper_tools`, from which the packaged binaries take Viper and Z3, so link
                # the Nix-built Viper there.
                ln -s ${packages.viper} viper_tools
                python3 x.py package release $out/bin
                rm -rf $out/target

                # Only Prusti's binaries are wrapped: the contracts' proc-macro library and
                # the executables in `viper_tools` must stay as they are.
                for f in $out/bin/{prusti-driver,prusti-rustc,cargo-prusti,prusti-server,prusti-server-driver}; do
                  wrapProgram $f \
                    --set RUST_SYSROOT "${rust}" \
                    --set JAVA_HOME "${pkgs.jdk11}/lib/openjdk" \
                    --set LD_LIBRARY_PATH "${pkgs.jdk11}/lib/openjdk/lib/server"
                done
              '';
            };
          };

          viper = pkgs.fetchzip {
            name = "viper";
            url = "https://viper.ethz.ch/downloads/ViperToolsNightlyLinux.zip";
            stripRoot = false;
            hash = "sha256-82vnyO7QWaLzehnBzPJxuEmdqK0MnWWwQnmdLq28sQc=";
          };
        };

        checks = {
          # prusti-test = naersk-lib.buildPackage {
          #   name = "prusti-test";
          #   version = "${prusti-version}";
          #   root = ./.;
          #   checkInputs = [
          #     pkgs.pkg-config
          #     pkgs.wget
          #     pkgs.gcc
          #     pkgs.openssl
          #     pkgs.jdk11
          #     packages.viper
          #   ];

          #   doCheck = true;

          #   override = _: {
          #     preBuild = ''
          #       export LD_LIBRARY_PATH="${pkgs.jdk11}/lib/openjdk/lib/server"
          #       export VIPER_HOME="${packages.viper}/backends"
          #       export Z3_EXE="${packages.viper}/z3/bin/z3"
          #     '';
          #     preCheck = ''
          #       export RUST_SYSROOT="${rust}"
          #       export JAVA_HOME="${pkgs.jdk11}/lib/openjdk"
          #       export LD_LIBRARY_PATH="${pkgs.jdk11}/lib/openjdk/lib/server"
          #       export VIPER_HOME="${packages.viper}/backends"
          #       export Z3_EXE="${packages.viper}/z3/bin/z3"
          #     '';
          #   };
          # };

          prusti-simple-test = pkgs.runCommand "prusti-simple-test" {
            buildInputs = [
              defaultPackage
              rust
            ];
          }
          ''
            cargo new --name example $out/example
            sed -i '1s/^/use prusti_contracts::*;\n/;s/println.*$/assert!(true);/' $out/example/src/main.rs
            cargo-prusti --manifest-path=$out/example/Cargo.toml
          '';
        };

        defaultPackage = packages.prusti;
      }
    );
}
