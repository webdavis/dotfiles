{
  description = ''
    Project development environment and dependency management flake.

    Provides:
      - Separate interactive and ad-hoc dev shells
      - treefmt (via treefmt-nix) as the single lint/format orchestrator
        ↪ Config: ./treefmt.nix — Ref: https://github.com/numtide/treefmt-nix
      - A `checks.treefmt` derivation so `nix flake check` fails on format drift
  '';

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt-nix.url = "github:numtide/treefmt-nix";
    treefmt-nix.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      treefmt-nix,
      ...
    }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-darwin" ] (
      system:
      let
        pkgs = import nixpkgs { inherit system; };

        treefmtEval = treefmt-nix.lib.evalModule pkgs ./treefmt.nix;

        baseShell = pkgs.mkShell {
          buildInputs = [
            treefmtEval.config.build.wrapper # `treefmt` with this repo's config baked in
            pkgs.bats # bats-core: test runner for the test/**/*.bats suites (`just test`)
            pkgs.chezmoi
            pkgs.just # so CI can run `nix develop .#run --command just test`
            pkgs.zizmor # GitHub Actions static analysis (`just lint-actions`)
          ];
        };

        interactiveShell = pkgs.mkShell {
          buildInputs = baseShell.buildInputs;
          shellHook = ''
            red="\e[91m"
            green="\e[32m"
            blue="\e[34m"
            bold="\e[1m"
            reset="\e[0m"

            projectName="$(basename "$PWD")"

            echo -e "''${blue}Entering dotfiles lint/dev environment...''${reset}\n"

            echo -e "''${bold}Project:''${reset} ''${green}''${projectName}''${reset}"

            echo -e "''${bold}Nix version:''${reset} ''${red}$(nix --version | cut -d' ' -f2-)''${reset}"
            echo -e "''${bold}treefmt version:''${reset} ''${red}$(treefmt --version | cut -d' ' -f2-)''${reset}"
            echo -e "''${bold}Bash version:''${reset} ''${red}$(bash --version | head -n 1)''${reset}"
          '';
        };
      in
      {
        devShells.default = interactiveShell;
        devShells.run = baseShell;
        formatter = treefmtEval.config.build.wrapper;
        checks.treefmt = treefmtEval.config.build.check self;
      }
    );
}
