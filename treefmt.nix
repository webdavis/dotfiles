# treefmt configuration, evaluated by treefmt-nix.lib.evalModule in flake.nix.
#
# This is the single lint/format orchestrator for the repo (it replaced the
# hand-rolled scripts/lint.sh). Style rules preserved from the old script:
#   - shell: shfmt -i 2 -ci -s; shellcheck (root .shellcheckrc disables SC1090/SC1091)
#   - markdown: mdformat + GFM plugin, honoring .mdformat.toml (105-column wrap)
#   - nix: nixfmt (RFC 166 style — pkgs.nixfmt is nixfmt-classic in nixpkgs 25.05,
#     so the package is overridden explicitly)
#   - TOML: taplo, with dot_aerospace.toml excluded (user-preferred visual alignment)
#   - JSON/YAML: validation-only via jq/yq (custom formatters below)
#
# Validators are treefmt "formatters" that never write; a non-zero exit fails the
# run — and the flake's treefmt check derivation, so `nix flake check` gates them.
{ pkgs, ... }:
let
  # `jq empty` per file: exit non-zero on any JSON parse error.
  jqValidate = pkgs.writeShellApplication {
    name = "jq-validate";
    runtimeInputs = [ pkgs.jq ];
    text = ''
      status=0
      for file do
        jq empty <"$file" || {
          echo "jq-validate: invalid JSON: $file" >&2
          status=1
        }
      done
      exit "$status"
    '';
  };

  # `yq eval` per file: exit non-zero on any YAML parse error.
  yqValidate = pkgs.writeShellApplication {
    name = "yq-validate";
    runtimeInputs = [ pkgs.yq-go ];
    text = ''
      status=0
      for file do
        yq eval '.' "$file" >/dev/null || {
          echo "yq-validate: invalid YAML: $file" >&2
          status=1
        }
      done
      exit "$status"
    '';
  };

in
{
  projectRootFile = "flake.nix";

  # The ONE global prune set (replaces lint.sh's 6x-duplicated `find -prune`
  # list). .git is never walked; the rest are belt-and-braces for the
  # filesystem walker (the git walker and the flake-source copy already skip
  # gitignored paths).
  settings.excludes = [
    ".direnv/**"
    ".worktrees/**"
    "**/vendor/**"
    "**/.vendor/**"
    "**/node_modules/**"
  ];

  # Shell — the old lint.sh shell set: *.sh, *.bash, dot_bash*, dot_profile
  # (never *.tmpl).
  programs.shellcheck.enable = true;
  programs.shellcheck.includes = [
    "*.sh"
    "*.bash"
    "dot_bash*"
    "dot_profile"
  ];
  programs.shellcheck.excludes = [ "*.tmpl" ];

  programs.shfmt.enable = true;
  # indent_size = 2 and simplify = true are the module defaults (-i 2 -s);
  # -ci (indent case labels) has no module option, so it is appended below.
  programs.shfmt.includes = [
    "*.sh"
    "*.bash"
    "dot_bash*"
    "dot_profile"
  ];
  programs.shfmt.excludes = [ "*.tmpl" ];
  settings.formatter.shfmt.options = [ "-ci" ];

  # Markdown — mdformat reads .mdformat.toml (wrap = 105, number = false,
  # end_of_line = "lf", validate = true) from the tree root; only the GFM
  # plugin needs wiring here.
  #
  # RULE: skill files (SKILL.md and any markdown shipped alongside a skill,
  # agent, or slash-command definition) are NEVER touched by mdformat.
  # Anthropic's skill-authoring guidance treats skills as authored prose with
  # no line-wrap or formatting requirement. There is also a mechanical reason:
  # skill/agent/command files rely on `---\nkey: value\n---` YAML frontmatter,
  # which mdformat (without the mdformat-frontmatter plugin) mangles into an
  # HR + H2 heading and breaks skill discovery. docs/superpowers/ also relies
  # on YAML frontmatter (specs and plans). docs/research/2026-04-12-worktrunk.md
  # fails mdformat's strict round-trip HTML validator (validate = true); the
  # exact GFM-table construct that trips it hasn't been isolated — quarantine
  # the one file so the rest of docs/research/ stays linted.
  programs.mdformat.enable = true;
  programs.mdformat.plugins = ps: [ ps.mdformat-gfm ];
  programs.mdformat.excludes = [
    "private_dot_claude/skills/**"
    "private_dot_claude/agents/**"
    "private_dot_claude/commands/**"
    "dot_agents/**"
    "docs/superpowers/**"
    "docs/research/2026-04-12-worktrunk.md"
  ];

  # Nix — RFC 166 style (same formatter nixfmt-tree wrapped before).
  programs.nixfmt.enable = true;
  programs.nixfmt.package = pkgs.nixfmt-rfc-style;

  # TOML — dot_aerospace.toml uses user-preferred visual alignment that
  # taplo's default formatter strips; skip it so the user's style is preserved.
  programs.taplo.enable = true;
  programs.taplo.excludes = [ "dot_aerospace.toml" ];

  # JSON validation. Chezmoi modify_ templates share the .json extension of
  # their target file but contain Go template directives, so jq can't parse
  # them.
  settings.formatter.jq-validate = {
    command = jqValidate;
    includes = [ "*.json" ];
    # Both spellings: treefmt matches patterns against the whole path from the
    # tree root, so bare "modify_*" only covers root-level files.
    excludes = [
      "modify_*"
      "**/modify_*"
    ];
  };

  # YAML validation — parity with lint.sh: only .chezmoidata is validated.
  settings.formatter.yq-validate = {
    command = yqValidate;
    includes = [
      ".chezmoidata/*.yaml"
      ".chezmoidata/*.yml"
    ];
  };
}
