# Pure (builtins-only) classifier for the rendered-template coverage discovery.
#
# This is the SINGLE source of truth for "which shell templates can render
# headless and are therefore safe to hand to the shellcheck-rendered-template
# formatter". treefmt.nix imports it to build that formatter's include list, and
# test/integration/rendered-template-coverage.sh drives the SAME functions through `nix eval`
# against the fixture matrix (beside a bash mirror), asserting agreement case by
# case. Before this file existed the Nix predicates were unexported let-bindings,
# so weakening one left every fixture green; now a weakened predicate here fails
# the fixture matrix directly.
#
# No `lib`, no `pkgs`: everything is `builtins`, so the test can import it with a
# bare `nix eval --impure` and no flake machinery. Every helper is textually
# mirrored by a bash function in the test; keep the two in lockstep.
rec {
  # File split into lines (drop the capture sublists builtins.split emits).
  fileLines = path: builtins.filter builtins.isString (builtins.split "\n" (builtins.readFile path));

  # One Go-template action `{{ ... }}`, QUOTE-AWARE: the body is a sequence of
  # double-quoted strings, backtick raw strings, or any character that is not a
  # `}`, `"`, or backtick. A `}` inside a quoted Go string is NOT structural, so
  # `{{ keepassxc "entry}name" }}` is one complete action (a naive `[^}]*` body
  # rejects it, sees no action at all, and would classify a secret template
  # safe). The quoted-string branches consume their content atomically, so a
  # quoted `}}` cannot close the action early either.
  actionRegex = "([{][{](\"[^\"]*\"|`[^`]*`|[^}\"`])*[}][}])";

  # Every Go-template action `{{ ... }}` on a line, as a list of the full action
  # strings (each including its `{{`/`}}` delimiters). builtins.split with a
  # capture group around the action leaves those matches as sublists (the first
  # capture is the whole action; take the head). An action that closes on the
  # same line is captured and a same-line sibling is a SEPARATE action (the
  # whole point: multiple actions per line are parsed individually, not
  # collapsed to the first or last).
  lineActions =
    line: map builtins.head (builtins.filter builtins.isList (builtins.split actionRegex line));

  # A line carries an UNTERMINATED action start when, after every complete
  # action is removed, a `{{` remains (e.g. a legal Go action split across
  # lines: `token={{ keepassxc` newline `"Entry" }}`). Such a line's action
  # content cannot be judged per line, so it is treated conservatively: the
  # file is NOT safely coverable (over-excluding a safe template is tolerable;
  # admitting a secret one never is). ONE tolerated shape: a leftover whose
  # FIRST `{{` opens a Go COMMENT (`{{/*` / `{{- /*`) is a multi-line comment
  # (the standard .chezmoitemplates partial-header preamble); its body never
  # renders, so it is not an action start. The anchor matters: the comment
  # opener must be the first `{{` in the leftover -- a leftover `{{` can only
  # be the line's unterminated TAIL (any later `}}` on the line would have
  # closed it), so a non-comment opener can never hide behind a later `{{/*`.
  lineHasUnterminatedActionStart =
    line:
    builtins.any (
      part:
      builtins.match ".*[{][{].*" part != null
      && builtins.match "([^{]|[{][^{])*[{][{]-?[[:space:]]*/[*].*" part == null
    ) (builtins.filter builtins.isString (builtins.split actionRegex line));

  # A Go-template COMMENT action (`{{/* ... */}}`, in any trim form).
  actionIsComment = action: builtins.match "[{][{]-?[[:space:]]*/[*].*" action != null;

  # A line invokes keepassxc when the identifier keepassxc/keepassxcAttribute
  # appears inside a NON-comment Go-template action on that line, not only as the
  # first token (`{{ $e := keepassxc "x" }}` counts). Each action is judged
  # INDIVIDUALLY: a leading comment action (`{{/* ... */}}`) on a line does not
  # suppress a real keepassxc action later on the SAME line, and a bare shell `#`
  # comment mentioning keepassxc carries no `{{` and so contributes no action.
  lineCallsKeepassxc =
    line:
    builtins.any (action: builtins.match ".*keepassxc.*" action != null && !(actionIsComment action)) (
      lineActions line
    );

  directCallsKeepassxc = path: builtins.any lineCallsKeepassxc (fileLines path);

  # Any line with an unterminated action start makes the whole file unsafe to
  # classify per line (the unclosed action's content lives on later lines).
  hasUnterminatedAction = path: builtins.any lineHasUnterminatedActionStart (fileLines path);

  # Parse a single ACTION for an includeTemplate directive
  # `{{ includeTemplate <arg> ... }}`. Returns null (not an include directive),
  # { name = "<literal>"; } for a double-quoted or backtick raw-string name, or
  # { dynamic = true; } when the name argument is not a static string literal (a
  # $var, (expr), or .field) and so cannot be resolved at eval time. Anchored on
  # the action's leading `{{`, so a comment action (`{{/* ... includeTemplate ...
  # */}}`) never matches and prose in a shell `#` comment (no `{{`) is not an
  # action at all.
  parseIncludeAction =
    action:
    let
      m = builtins.match "[{][{]-?[[:space:]]*includeTemplate[[:space:]]+(.*)" action;
    in
    if m == null then
      null
    else
      let
        rest = builtins.head m;
        dq = builtins.match "\"([^\"]*)\".*" rest;
        bt = builtins.match "`([^`]*)`.*" rest;
      in
      if dq != null then
        { name = builtins.head dq; }
      else if bt != null then
        { name = builtins.head bt; }
      else
        { dynamic = true; };

  # All includeTemplate directives in a file: { dynamic = bool; names = [str]; }.
  # EVERY action on EVERY line is parsed, so multiple includes on one line (in
  # either order, dynamic or literal) are all accounted for.
  includeDirectives =
    path:
    let
      actions = builtins.concatLists (map lineActions (fileLines path));
      hits = builtins.filter (x: x != null) (map parseIncludeAction actions);
    in
    {
      dynamic = builtins.any (h: h ? dynamic) hits;
      names = map (h: h.name) (builtins.filter (h: h ? name) hits);
    };

  # A template cannot render headless (is UNSAFE) when it, or any partial it
  # includeTemplates (transitively, literal names resolved against includeBase),
  # calls keepassxc, OR carries an unterminated (multi-line) action start
  # (conservative: its content cannot be judged per line), OR when any
  # includeTemplate name cannot be resolved statically (conservative
  # rejection). Cycle-protected: a partial already on the current include chain
  # contributes no new unsafety, so a cyclic pair terminates. A literal include
  # whose partial is absent under includeBase is skipped (it would fail at real
  # render time, a separate concern).
  rendersUnsafe =
    includeBase: path:
    let
      go =
        visited: p:
        if builtins.elem p visited then
          false
        else if directCallsKeepassxc p || hasUnterminatedAction p then
          true
        else
          let
            incs = includeDirectives p;
          in
          if incs.dynamic then
            true
          else
            builtins.any (
              name:
              let
                partial = includeBase + "/${name}";
              in
              builtins.pathExists partial && go (visited ++ [ p ]) partial
            ) incs.names;
    in
    go [ ] path;

  # A template is a shell template when its first line is a shell shebang (or a
  # `# shellcheck shell=` directive), OR its first line is a Go-template
  # directive and its first non-directive line is such a shebang (the
  # osquery-loader shape).
  isShellShebangLine =
    line: builtins.match "#!.*sh.*" line != null || builtins.match "# shellcheck shell=.*" line != null;
  isGoDirectiveLine = line: builtins.match "[[:space:]]*[{][{].*" line != null;
  isShellTemplate =
    path:
    let
      ls = fileLines path;
      first = if ls == [ ] then "" else builtins.head ls;
      nonDirective = builtins.filter (l: !(isGoDirectiveLine l)) ls;
      firstNonDirective = if nonDirective == [ ] then "" else builtins.head nonDirective;
    in
    isShellShebangLine first || (isGoDirectiveLine first && isShellShebangLine firstNonDirective);

  isShTmpl = path: builtins.match ".*\\.sh\\.tmpl" (builtins.baseNameOf path) != null;

  # The coverage verdict for a template: "covered" when it is a shell template
  # (a *.sh.tmpl, or shell-classified by shape) that can render headless;
  # "excluded" otherwise. Mirrors the bash fixture oracle in the test.
  classify =
    includeBase: path:
    if rendersUnsafe includeBase path then
      "excluded"
    else if isShTmpl path || isShellTemplate path then
      "covered"
    else
      "excluded";
}
