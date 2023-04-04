{
  description = "Nix package for centrifuge-chain";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixos-21.11;
    flake-utils = {
        url = github:numtide/flake-utils;
        inputs.nixpkgs.follows = "nixpkgs";
    };
    gitignore = {
      url = github:hercules-ci/gitignore.nix;
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = github:nix-community/fenix;
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = inputs :
    inputs.flake-utils.lib.eachDefaultSystem (system:
        let
          pkgs = inputs.nixpkgs.legacyPackages.${system};

          cargoTOML = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          rustToolChainTOML = builtins.fromTOML (builtins.readFile ./rust-toolchain.toml);

          name = cargoTOML.package.name;
          # This is the program version.
          version = cargoTOML.package.version;
          # This selects a nightly Rust version, based on the date.
          nightly-date = pkgs.lib.strings.removePrefix "nightly-" rustToolChainTOML.toolchain.channel;
          # This is the hash of the Rust toolchain at nightly-date, required for reproducibility.
          nightly-sha256 = "sha256-UuVX3RxSsUfng4G/Bec8JcI/lOUmxrG7NXSG5hMRgbc=";
          # This is the git short commit of the current version of the program.
          shortCommit = builtins.substring 0 7 (inputs.self.rev or "dirty");

          # This instantiates a new Rust version based on nightly-date.
          nightlyRustPlatform = pkgs.makeRustPlatform {
            inherit
              (inputs.fenix.packages.${system}.toolchainOf {
                channel = "nightly";
                date = nightly-date;
                sha256 = nightly-sha256;
              })
              cargo rustc;
          };

          # This is a mock git program, which just returns the commit-substr value.
          # It is called when the build process calls git. Instead of the real git,
          # it will find this one.
          git-mock =
            pkgs.writeShellScriptBin "git" ''
              echo ${shortCommit}
            '';

          # srcFilter is used to keep out of the build non-source files,
          # so that we only trigger a rebuild when necessary.
          srcFilter = src:
            let
              isGitIgnored = inputs.gitignore.lib.gitignoreFilter src;

              ignoreList = [
                ".dockerignore"
                ".envrc"
                ".github"
                ".travis.yml"
                "CODE_OF_CONDUCT.md"
                "README.md"
                "ci"
                "cloudbuild.yaml"
                "codecov.yml"
                "docker-compose.yml"
                "rustfmt.toml"
              ];
            in
            path: type:
              isGitIgnored path type
              && builtins.all (name: builtins.baseNameOf path != name) ignoreList;
        in
        rec {
          defaultPackage = nightlyRustPlatform.buildRustPackage {
            pname = name;
            inherit version;
            inherit shortCommit;

            # This applies the srcFilter function to the current directory, so
            # we don't include unnecessary files in the package.
            src = pkgs.lib.cleanSourceWith {
              src = ./.;
              filter = srcFilter ./.;
              name = "${name}-source";
            };

            # This is a hash of all the Cargo dependencies, for reproducibility.
            cargoSha256 = "sha256-B44+vaseMmcmEAn0qhGime0a1geVofH0Qtbd9Epo5KI=";

            nativeBuildInputs = with pkgs; [ clang git-mock pkg-config ];
            buildInputs = with pkgs; [ openssl ] ++ (
                 lib.optionals stdenv.isDarwin [
                   darwin.apple_sdk.frameworks.Security
                   darwin.apple_sdk.frameworks.SystemConfiguration
                 ]
            );

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            PROTOC = "${pkgs.protobuf}/bin/protoc";
            SKIP_WASM_BUILD = 1;


            doCheck = false;
          };

          packages.fastRuntime = defaultPackage.overrideAttrs (base: {
            buildFeatures = [ "fast-runtime" ];
          });

          # Docker image package doesn't work on Darwin Archs
          packages.dockerImage = pkgs.dockerTools.buildLayeredImage {
            name = "centrifugeio/${name}";
            tag = "${version}-${shortCommit}-nix-do-not-use"; # todo remove suffix once verified
            # This uses the date of the last commit as the image creation date.
            created = builtins.substring 0 8 inputs.self.lastModifiedDate;

            contents = [
              pkgs.busybox
              inputs.self.defaultPackage.${system}
            ];

            config = {
              ExposedPorts = {
                "30333/tcp" = { };
                "9933/tcp" = { };
                "9944/tcp" = { };
              };
              Volumes = {
                "/data" = { };
              };
              Entrypoint = [ "centrifuge-chain" ];
            };
          };

          packages.dockerImageFastRuntime = packages.dockerImage.overrideAttrs (base: {
            tag = "test-${version}-${shortCommit}-nix-do-not-use"; # todo remove suffix once verified
            contents = [
                pkgs.busybox
                packages.fastRuntime
            ];
          });
    });
}
