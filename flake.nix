{
  description = "Nix package for centrifuge-chain";

  inputs = {
    nixpkgs.url = github:NixOS/nixpkgs/nixos-20.09;
  };

  outputs = inputs:
    let
      name = "centrifuge-chain";
      version = "2.0.0";
      pkgs = inputs.nixpkgs.legacyPackages.x86_64-linux;

      # this mocks git to return the truncated SHA hash we get from Nix
      git-mock = pkgs.writeShellScriptBin "git" ''
        echo ${builtins.substring 0 6 (inputs.self.rev or "dirty")}
      '';

      # srcFilter is used to keep out of the build non-source files,
      # so that we only trigger a rebuild when necessary.
      srcFilter = path: type:
        let
          p = baseNameOf path;
        in
          !(
            # ignore CI directories
            (type == "directory" && (p == ".github" || p == "ci")) ||
            # ignore CI files
            p == ".travis.yml" || p == "cloudbuild.yaml" ||
            # ignore flake.(nix|lock)
            p == "flake.nix" || p == "flake.lock" ||
            # ignore docker files
            p == ".dockerignore" || p == "docker-compose.yml" ||
            # ignore misc
            p == "rustfmt.toml"
          );

    in
    {
      packages.x86_64-linux.centrifuge-chain =
        pkgs.rustPlatform.buildRustPackage {
          pname = name;
          version = version;

          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = srcFilter;
            name = "centrifuge-chain-source";
          };
          cargoSha256 = "sha256-52CN7N9FQiJSODloo0VZGPNw4P5XsaWfaQxEf6Nm2gI=";

          nativeBuildInputs = with pkgs; [ clang git-mock pkg-config ];
          buildInputs = with pkgs; [ openssl ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang}/lib";
          PROTOC = "${pkgs.protobuf}/bin/protoc";
          BUILD_DUMMY_WASM_BINARY = 1;

          doCheck = false;
        };

      defaultPackage.x86_64-linux = inputs.self.packages.x86_64-linux.centrifuge-chain;

      packages.x86_64-linux.dockerContainer =
        pkgs.dockerTools.buildImage {
          name = "centrifugeio/${name}";
          tag = "latest";

          contents = inputs.self.defaultPackage.x86_64-linux;

          config = {
            Env = [
              "PATH=/bin:$PATH"
            ];
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
