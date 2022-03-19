{
  description = "Nix package for centrifuge-chain";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixos-21.11;
    gitignore = {
      url = github:hercules-ci/gitignore.nix;
      inputs.nixpkgs.follows = "nixpkgs";
    };
    fenix = {
      url = github:nix-community/fenix;
      inputs.nixpkgs.follows = "nixpkgs";
    };

  };

  outputs = inputs:
    let
      name = "centrifuge-chain";
      # This is the program version.
      version = "0.10.9";
      # This selects a nightly Rust version, based on the date.
      nightly-date = "2021-11-07";
      # This is the hash of the Rust toolchain at nightly-date, required for reproducibility.
      nightly-sha256 = "sha256-eENaPaU6wpk8qAUg5PJPpvAmSVQNCbOI3ipUMhHmwXk=";

      # This could be made into a list, to support multiple platforms
      system = "x86_64-linux";

      pkgs = inputs.nixpkgs.legacyPackages.${system};

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
        let
          # This evaluates to the first 7 digits of the git hash of this repo's HEAD
          # commit, or to "dirty" if there are uncommitted changes.
          commit-substr = builtins.substring 0 7 (inputs.self.rev or "dirty");
        in
        pkgs.writeShellScriptBin "git" ''
          echo ${commit-substr}
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
            "flake.lock"
            "flake.nix"
            "rustfmt.toml"
          ];
        in
        path: type:
          isGitIgnored path type
          && builtins.all (name: builtins.baseNameOf path != name) ignoreList;
    in
    {
      defaultPackage.${system} = nightlyRustPlatform.buildRustPackage {
        pname = name;
        inherit version;

        # This applies the srcFilter function to the current directory, so
        # we don't include unnecessary files in the package.
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = srcFilter ./.;
          name = "${name}-source";
        };

        # This is a hash of all the Cargo dependencies, for reproducibility.
        cargoSha256 = "sha256-ulzzofKBqw4RUwwBmFKvgfCZ1ZeuULvCHLEQVzZrKBk=";

        nativeBuildInputs = with pkgs; [ clang git-mock pkg-config ];
        buildInputs = [ pkgs.openssl ];

        LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        PROTOC = "${pkgs.protobuf}/bin/protoc";
        SKIP_WASM_BUILD = 1;


        doCheck = false;
      };

      packages.${system}.dockerImage = pkgs.dockerTools.buildLayeredImage {
        name = "centrifugeio/${name}";
        tag = version;
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
    };
}
