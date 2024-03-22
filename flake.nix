{
  description = "Espresso Decentralized Sequencer";

  nixConfig = {
    extra-substituters = [
      "https://espresso-systems-private.cachix.org"
      "https://nixpkgs-cross-overlay.cachix.org"
    ];
    extra-trusted-public-keys = [
      "espresso-systems-private.cachix.org-1:LHYk03zKQCeZ4dvg3NctyCq88e44oBZVug5LpYKjPRI="
      "nixpkgs-cross-overlay.cachix.org-1:TjKExGN4ys960TlsGqNOI/NBdoz2Jdr2ow1VybWV5JM="
    ];
  };

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.rust-overlay.url = "github:oxalica/rust-overlay";

  inputs.fenix.url = "github:nix-community/fenix";
  inputs.fenix.inputs.nixpkgs.follows = "nixpkgs";

  inputs.nixpkgs-cross-overlay.url =
    "github:alekseysidorov/nixpkgs-cross-overlay";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  inputs.foundry.url =
    "github:shazow/foundry.nix/monthly"; # Use monthly branch for permanent releases
  inputs.solc-bin.url = "github:EspressoSystems/nix-solc-bin";

  inputs.flake-compat.url = "github:edolstra/flake-compat";
  inputs.flake-compat.flake = false;

  inputs.pre-commit-hooks.url = "github:cachix/pre-commit-hooks.nix";

  outputs =
    { self
    , nixpkgs
    , rust-overlay
    , nixpkgs-cross-overlay
    , flake-utils
    , pre-commit-hooks
    , fenix
    , foundry
    , solc-bin
    , ...
    }:
    flake-utils.lib.eachDefaultSystem (system:
    let
      # node=error: disable noisy anvil output
      RUST_LOG = "info";
      RUST_BACKTRACE = 1;
      # Use a distinct target dir for builds from within nix shells.
      CARGO_TARGET_DIR = "target/nix";

      solhintPkg = { buildNpmPackage, fetchFromGitHub }:
        buildNpmPackage rec {
          pname = "solhint";
          version = "4.5.2";
          src = fetchFromGitHub {
            owner = "protofire";
            repo = pname;
            rev = "v.${version}";
            hash = "sha256-LaOEs1pSr7jtabyqadv12Lq30C7yPXhkuZRhpjDQuv4=";
          };
          npmDepsHash = "sha256-dNweOrXTS5lmnj7odCZsChysSYrWYRIPHk4KO1HVTG4=";
          dontNpmBuild = true;
        };

      overlays = [
        (import rust-overlay)
        foundry.overlay
        solc-bin.overlays.default
        (final: prev: {
          solhint =
            solhintPkg { inherit (prev) buildNpmPackage fetchFromGitHub; };
        })
      ];
      pkgs = import nixpkgs { inherit system overlays; };
      crossShell = { config }:
        let
          localSystem = system;
          crossSystem = {
            inherit config;
            useLLVM = true;
            isStatic = true;
          };
          pkgs = import "${nixpkgs-cross-overlay}/utils/nixpkgs.nix" {
            inherit overlays localSystem crossSystem;
          };
        in
        import ./cross-shell.nix {
          inherit pkgs;
          inherit RUST_LOG RUST_BACKTRACE CARGO_TARGET_DIR;
        };
    in
    with pkgs; {
      checks = {
        pre-commit-check = pre-commit-hooks.lib.${system}.run {
          src = ./.;
          hooks = {
            cargo-fmt = {
              enable = true;
              description = "Enforce rustfmt";
              entry = "cargo fmt --all";
              types_or = [ "rust" "toml" ];
              pass_filenames = false;
            };
            cargo-sort = {
              enable = true;
              description = "Ensure Cargo.toml are sorted";
              entry = "cargo sort -g -w";
              types_or = [ "toml" ];
              pass_filenames = false;
            };
            cargo-clippy = {
              enable = true;
              description = "Run clippy";
              entry =
                "cargo clippy --all-features --all-targets -- -D warnings";
              types_or = [ "rust" "toml" ];
              pass_filenames = false;
            };
            forge-fmt = {
              enable = true;
              description = "Enforce forge fmt";
              entry = "forge fmt";
              types_or = [ "solidity" ];
              pass_filenames = false;
            };
            solhint = {
              enable = true;
              description = "Solidity linter";
              entry = "solhint --fix 'contracts/{script,src,test}/**/*.sol'";
              types_or = [ "solidity" ];
              pass_filenames = true;
            };
            contract-bindings = {
              enable = true;
              description = "Generate contract bindings";
              entry = "just gen-bindings";
              types_or = [ "solidity" ];
              pass_filenames = false;
            };
            prettier-fmt = {
              enable = true;
              description = "Enforce markdown formatting";
              entry = "prettier -w";
              types_or = [ "markdown" ];
              pass_filenames = true;
            };
            spell-checking = {
              enable = true;
              description = "Spell checking";
              entry = "typos";
              pass_filenames = true;
            };
            nixpkgs-fmt.enable = true;
          };
        };
      };

      devShells =
        let
          mkRustShell = { toolchain, extraPkgs ? [ ], extraEnv ? { }, extraShellHook ? "" }:
            (mkShell {
              packages = with pkgs; [
                # Rust dependencies
                pkg-config
                openssl
                curl
                toolchain
              ] ++ extraPkgs;
              shellHook = ''
                # Prevent cargo aliases from using programs in `~/.cargo` to avoid conflicts
                # with rustup installations.
                export CARGO_HOME=$HOME/.cargo-nix
                export PATH="$PWD/$CARGO_TARGET_DIR/release:$PATH"
              '' + extraShellHook;
              RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
              inherit RUST_LOG RUST_BACKTRACE CARGO_TARGET_DIR;
            }).overrideAttrs
              (old: extraEnv);
          stableToolchain = pkgs.rust-bin.stable.latest.minimal.override {
            extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
          };
          nightlyToolchain = pkgs.rust-bin.nightly.latest.minimal.override {
            extensions = [ "rustfmt" "clippy" "llvm-tools-preview" "rust-src" ];
          };
        in
        {
          default = let solc = pkgs.solc-bin.latest; in
            mkRustShell {
              toolchain = stableToolchain;
              extraEnv = { FOUNDRY_SOLC = "${solc}/bin/solc"; };
              extraShellHook = self.checks.${system}.pre-commit-check.shellHook;
              extraPkgs = with pkgs; [
                # Rust tools
                cargo-audit
                cargo-edit
                cargo-sort
                typos
                just
                fenix.packages.${system}.rust-analyzer

                # Tools
                nixpkgs-fmt

                # Ethereum contracts, solidity, ...
                foundry-bin
                solc
                nodePackages.prettier
                solhint
              ] ++ lib.optionals stdenv.isDarwin
                [ darwin.apple_sdk.frameworks.SystemConfiguration ]
              ++ lib.optionals (!stdenv.isDarwin) [ cargo-watch ] # broken on OSX
              ;
            };
          nightly = mkRustShell { toolchain = nightlyToolchain; };
          crossShell = crossShell { config = "x86_64-unknown-linux-musl"; };
          armCrossShell = crossShell { config = "aarch64-unknown-linux-musl"; };
        };
    }
    );
}
