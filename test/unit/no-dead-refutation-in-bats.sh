#!/usr/bin/env bash
# no-dead-refutation-in-bats.sh, a REPO-WIDE guard.
#
# THE PROPERTY: inside a bats-executed body (@test, setup, teardown, and their
# file/suite variants), a bare inverted command's exit status must be ABLE to
# fail the test by some path. bash's `set -e` and bats' ERR trap both ignore a
# `!` pipeline, and bats takes the body's LAST status as the result, so an
# inverted command decides the test ONLY as the last command the body executes.
# Everywhere else it reads like a working check and cannot fail.
#
# The mechanism, measured rather than assumed (each shape was run under the
# repo's bats with a VIOLATED refutation; "dead" means the test still passed):
#
#   ! cmd                       final statement        LIVE  (decides the test)
#   other; ! cmd                final line             LIVE  (still executes last)
#   ! cmd; other                anywhere               DEAD  (status discarded)
#   other; ! cmd                non-final line         DEAD
#   ! cmd &                     anywhere, even final   DEAD  (wait cannot recover it)
#   x && ! cmd / x || ! cmd     non-final statement    DEAD  (the list returns the
#                                                      exempt inverted status)
#   ! cmd && x  (no || after)   non-final statement    DEAD  (violation short-
#                                                      circuits to that status)
#   ! cmd || handler            anywhere               LIVE  (handler failure is
#                                                      NOT exempt)
#   { ! cmd; } / if,loop bodies non-final statement    DEAD  (the exemption
#                                                      propagates through
#                                                      non-subshell compounds)
#   time ! cmd                  non-final statement    DEAD  (time is pipeline
#                                                      syntax)
#   ( ! cmd ) / x=$(! cmd)      anywhere               LIVE  (a subshell or
#                                                      substitution surfaces the
#                                                      status to errexit)
#   f() { ! cmd; }; f           anywhere               LIVE  (the call is a plain
#                                                      command)
#   if ! cmd; then / while !    anywhere               LIVE  (condition
#                                                      consumption)
#   [[ ! -e x ]]                anywhere               LIVE  (fails via the [[
#                                                      compound, which errexit
#                                                      sees)
#
# So the guard flags an inverted pipeline when its status is discarded: it is
# backgrounded, or it sits outside the body's final top-level statement with
# nothing consuming it (no condition, no following `||` handler, no enclosing
# subshell or substitution, not a function-definition body).
#
# CLAIMED EXACTLY, NO MORE -- the scan's known limits, each pinned as a
# boundary fixture in test/test-system/dead-refutation-shapes.sh:
#   - inside the body's FINAL compound statement (e.g. `{ ! cmd; true; }` as
#     the last statement) the inversion is presumed live, because which inner
#     command runs last is data-dependent;
#   - case...esac bodies are scanned opaquely, so a dead inversion in a case
#     branch passes;
#   - parenthesized contexts are presumed consumed, so a dead inversion in a
#     process substitution passes;
#   - function BODIES are exempt (calls are live; a multi-command body hiding
#     a dead inversion passes), and file-scope helper functions other than the
#     bats setup/teardown family are not scanned at all;
#   - heredocs inside command substitutions are not tracked.
#
# Why a guard and not just the fix: a dead assertion is invisible in a green
# run. It reads as coverage, it costs a reviewer real attention to spot, and
# the next test copied from a neighbour inherits it. Nine such assertions were
# live on main, including one asserting that alert data is never sent off-box.
#
# Usage: no-dead-refutation-in-bats.sh [scan-root]
# Default scan root is this repo's test/ directory; the test-system suite
# points it at scratch fixture trees.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCAN_ROOT="${1:-$REPO_ROOT/test}"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -d $SCAN_ROOT ]] || fail "no such scan root: $SCAN_ROOT"

report="$(
  python3 - "$SCAN_ROOT" <<'PY'
import os, re, sys

scan_root = sys.argv[1]

BODY_OPENERS = {"setup", "teardown", "setup_file", "teardown_file",
                "setup_suite", "teardown_suite"}
COMPOUND_OPEN = {"if", "for", "while", "until"}
FUNCDEF_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*\(\)$")


class Lexer:
    """Tokenize shell text into WORD / OP / NL / PARENGROUP tokens, skipping
    comments, quoted text, heredoc bodies, ${...}, $(...), `...`, <(...),
    >(...), and [[ ... ]] (each swallowed opaquely into its word)."""

    def __init__(self, text):
        self.text = text
        self.n = len(text)
        self.i = 0
        self.line = 1
        self.heredocs = []  # pending (delimiter, strip_tabs)

    def error(self, message):
        raise SyntaxError("line %d: %s" % (self.line, message))

    def take(self):
        ch = self.text[self.i]
        self.i += 1
        if ch == "\n":
            self.line += 1
        return ch

    def peek(self, k=0):
        j = self.i + k
        return self.text[j] if j < self.n else ""

    def skip_single_quote(self):  # after the opening '
        while self.i < self.n:
            if self.take() == "'":
                return
        self.error("unterminated single quote")

    def skip_ansi_quote(self):  # after the opening $'
        while self.i < self.n:
            ch = self.take()
            if ch == "\\" and self.i < self.n:
                self.take()
            elif ch == "'":
                return
        self.error("unterminated $'...' quote")

    def skip_double_quote(self):  # after the opening "
        while self.i < self.n:
            ch = self.take()
            if ch == "\\" and self.i < self.n:
                self.take()
            elif ch == "`":
                self.skip_backtick()
            elif ch == "$" and self.peek() in "({":
                opener = self.take()
                self.skip_matched(opener, ")" if opener == "(" else "}")
            elif ch == '"':
                return
        self.error("unterminated double quote")

    def skip_backtick(self):  # after the opening `
        while self.i < self.n:
            ch = self.take()
            if ch == "\\" and self.i < self.n:
                self.take()
            elif ch == "`":
                return
        self.error("unterminated backtick")

    def skip_matched(self, open_ch, close_ch):
        """After an opening delimiter: skip to its match, quote-aware."""
        depth = 1
        while self.i < self.n:
            ch = self.take()
            if ch == "\\" and self.i < self.n:
                self.take()
            elif ch == "'":
                self.skip_single_quote()
            elif ch == '"':
                self.skip_double_quote()
            elif ch == "`":
                self.skip_backtick()
            elif ch == "$" and self.peek() in "({'":
                opener = self.take()
                if opener == "'":
                    self.skip_ansi_quote()
                else:
                    self.skip_matched(opener, ")" if opener == "(" else "}")
            elif ch == open_ch:
                depth += 1
            elif ch == close_ch:
                depth -= 1
                if depth == 0:
                    return
        self.error("unterminated %s...%s" % (open_ch, close_ch))

    def skip_dbrackets(self):  # after the word [[
        while self.i < self.n:
            ch = self.take()
            if ch == "\\" and self.i < self.n:
                self.take()
            elif ch == "'":
                self.skip_single_quote()
            elif ch == '"':
                self.skip_double_quote()
            elif ch == "$" and self.peek() in "({":
                opener = self.take()
                self.skip_matched(opener, ")" if opener == "(" else "}")
            elif ch == "]" and self.peek() == "]":
                self.take()
                return
        self.error("unterminated [[ ... ]]")

    def consume_pending_heredocs(self):
        """Called just after a newline: swallow heredoc bodies line by line."""
        while self.heredocs:
            delimiter, strip_tabs = self.heredocs.pop(0)
            while self.i < self.n:
                start = self.i
                while self.i < self.n and self.peek() != "\n":
                    self.take()
                line_text = self.text[start:self.i]
                if self.i < self.n:
                    self.take()  # the newline
                candidate = line_text.lstrip("\t") if strip_tabs else line_text
                if candidate == delimiter:
                    break

    def read_heredoc_delimiter(self):
        """After <<: parse the (possibly quoted) delimiter word."""
        strip_tabs = False
        if self.peek() == "-":
            self.take()
            strip_tabs = True
        while self.peek() in " \t":
            self.take()
        delimiter = ""
        while self.i < self.n and self.peek() not in " \t\n;&|<>()":
            ch = self.take()
            if ch == "\\" and self.i < self.n:
                delimiter += self.take()
            elif ch == "'":
                start = self.i
                self.skip_single_quote()
                delimiter += self.text[start:self.i - 1]
            elif ch == '"':
                start = self.i
                self.skip_double_quote()
                delimiter += self.text[start:self.i - 1]
            else:
                delimiter += ch
        self.heredocs.append((delimiter, strip_tabs))

    def tokens(self):
        """Yield (kind, value, line): kind in WORD, OP, NL, PARENGROUP."""
        word = ""
        word_line = None

        def flush():
            nonlocal word, word_line
            if word:
                token = ("WORD", word, word_line)
                word = ""
                word_line = None
                return token
            return None

        while self.i < self.n:
            ch = self.peek()
            if ch == "\\":
                if self.peek(1) == "\n":  # line continuation: plain whitespace
                    token = flush()
                    if token:
                        yield token
                    self.take()
                    self.take()
                    continue
                if word_line is None:
                    word_line = self.line
                word += self.take()
                if self.i < self.n:
                    word += self.take()
                continue
            if ch in "'\"`":
                if word_line is None:
                    word_line = self.line
                start = self.i
                self.take()
                if ch == "'":
                    self.skip_single_quote()
                elif ch == '"':
                    self.skip_double_quote()
                else:
                    self.skip_backtick()
                word += self.text[start:self.i]
                continue
            if ch == "$":
                if word_line is None:
                    word_line = self.line
                start = self.i
                self.take()
                nxt = self.peek()
                if nxt == "(":
                    self.take()
                    self.skip_matched("(", ")")
                elif nxt == "{":
                    self.take()
                    self.skip_matched("{", "}")
                elif nxt == "'":
                    self.take()
                    self.skip_ansi_quote()
                elif nxt == '"':
                    self.take()
                    self.skip_double_quote()
                word += self.text[start:self.i]
                continue
            if ch == "#" and not word:
                while self.i < self.n and self.peek() != "\n":
                    self.take()
                continue
            if ch == "\n":
                token = flush()
                if token:
                    yield token
                line = self.line
                self.take()
                self.consume_pending_heredocs()
                yield ("NL", "\n", line)
                continue
            if ch in " \t":
                token = flush()
                if token:
                    yield token
                self.take()
                continue
            if ch == ";":
                token = flush()
                if token:
                    yield token
                line = self.line
                self.take()
                value = ";"
                while self.peek() in ";&":  # ;; ;& ;;& all end a list
                    value += self.take()
                yield ("OP", value, line)
                continue
            if ch == "&":
                if self.peek(1) == "&":
                    token = flush()
                    if token:
                        yield token
                    line = self.line
                    self.take()
                    self.take()
                    yield ("OP", "&&", line)
                elif self.peek(1) == ">":
                    if word_line is None:
                        word_line = self.line
                    word += self.take() + self.take()  # &> redirection
                else:
                    token = flush()
                    if token:
                        yield token
                    line = self.line
                    self.take()
                    yield ("OP", "&", line)
                continue
            if ch == "|":
                token = flush()
                if token:
                    yield token
                line = self.line
                self.take()
                value = "|"
                if self.peek() in "|&":
                    value += self.take()
                yield ("OP", value, line)
                continue
            if ch == "<":
                if self.peek(1) == "<" and self.peek(2) == "<":
                    if word_line is None:
                        word_line = self.line
                    word += self.take() + self.take() + self.take()
                    continue
                if self.peek(1) == "<":
                    token = flush()
                    if token:
                        yield token
                    self.take()
                    self.take()
                    self.read_heredoc_delimiter()
                    continue
                if self.peek(1) == "(":
                    if word_line is None:
                        word_line = self.line
                    start = self.i
                    self.take()
                    self.take()
                    self.skip_matched("(", ")")
                    word += self.text[start:self.i]
                    continue
                if word_line is None:
                    word_line = self.line
                word += self.take()
                continue
            if ch == ">":
                if self.peek(1) == "(":
                    if word_line is None:
                        word_line = self.line
                    start = self.i
                    self.take()
                    self.take()
                    self.skip_matched("(", ")")
                    word += self.text[start:self.i]
                    continue
                if word_line is None:
                    word_line = self.line
                word += self.take()
                if self.peek() in ">&":
                    word += self.take()
                continue
            if ch == "(":
                if word:
                    # Glued to a word: array literal x=(...) or funcdef f().
                    start = self.i
                    self.take()
                    self.skip_matched("(", ")")
                    word += self.text[start:self.i]
                    continue
                line = self.line
                start = self.i
                self.take()
                self.skip_matched("(", ")")
                yield ("PARENGROUP", self.text[start:self.i], line)
                continue
            if ch == ")":
                # A stray closer: a case-branch pattern terminator. Tolerated;
                # case bodies are analyzed opaquely.
                token = flush()
                if token:
                    yield token
                self.take()
                continue
            # A plain word character.
            if word_line is None:
                word_line = self.line
            word += self.take()
            # A completed [[ word triggers an opaque swallow to ]].
            if word == "[[" and self.peek() in " \t\n":
                line = word_line
                self.skip_dbrackets()
                yield ("WORD", "[[...]]", line)
                word = ""
                word_line = None
        token = flush()
        if token:
            yield token


class BodyAnalyzer:
    """Positional analysis of one bats-executed body's token stream."""

    CONTINUATION_OPS = {"&&", "||", "|", "|&"}

    def __init__(self, relpath, lines):
        self.relpath = relpath
        self.lines = lines
        self.frames = []      # {"kind": group|funcdef|if|loop, "cond": bool}
        self.statement = 0    # top-level statement counter
        self.need_new_statement = True
        self.at_command_position = True
        self.case_depth = 0   # inside case...esac: analyzed opaquely
        self.open_inversions = []  # sites not yet terminated
        self.candidates = []       # closed sites pending the final-statement check
        self.flagged = []          # (line, reason) definite findings
        self.pending_funcdef = False
        self.previous_word = None
        self.last_significant = None

    def in_condition(self):
        return bool(self.frames) and self.frames[-1].get("cond", False)

    def in_funcdef(self):
        return any(frame["kind"] == "funcdef" for frame in self.frames)

    def open_inversion(self, line):
        self.open_inversions.append({
            "line": line,
            "level": len(self.frames),
            "statement": self.statement,
            "or_after": False,
            "cond": self.in_condition(),
            "funcdef": self.in_funcdef(),
        })

    def close_inversions(self, backgrounded):
        level = len(self.frames)
        remaining = []
        for inversion in self.open_inversions:
            if inversion["level"] != level:
                remaining.append(inversion)
                continue
            if inversion["funcdef"] or inversion["cond"] or inversion["or_after"]:
                continue
            if backgrounded:
                self.flagged.append((inversion["line"], "backgrounded"))
            else:
                self.candidates.append(inversion)
        self.open_inversions = remaining

    def begin_statement_if_needed(self):
        if not self.frames and self.need_new_statement:
            self.statement += 1
            self.need_new_statement = False

    def feed(self, kind, value, line):
        if self.case_depth > 0:
            # Opaque case scan: only track nested case/esac words.
            if kind == "WORD":
                if value == "case":
                    self.case_depth += 1
                elif value == "esac":
                    self.case_depth -= 1
                    if self.case_depth == 0:
                        self.at_command_position = False
                        self.last_significant = value
            return
        if kind == "NL":
            if self.last_significant in self.CONTINUATION_OPS:
                return  # the list continues on the next line
            self.close_inversions(backgrounded=False)
            if not self.frames:
                self.need_new_statement = True
            self.at_command_position = True
            return
        if kind == "OP":
            self.last_significant = value
            self.previous_word = None
            self.pending_funcdef = False
            if value == "&&":
                # Violation short-circuits to the exempt inverted status, so
                # && does NOT consume it (measured).
                pass
            elif value == "||":
                for inversion in self.open_inversions:
                    if inversion["level"] == len(self.frames):
                        inversion["or_after"] = True
            elif value in ("|", "|&"):
                pass  # same pipeline: the leading ! still covers it
            else:  # ; ;; ;& ;;& &
                backgrounded = value == "&"
                self.close_inversions(backgrounded)
                if not self.frames:
                    self.need_new_statement = True
            self.at_command_position = True
            return

        # WORD / PARENGROUP tokens.
        self.last_significant = value
        if kind == "WORD" and self.at_command_position:
            if value == "!":
                self.begin_statement_if_needed()
                self.open_inversion(line)
                self.previous_word = "!"
                self.pending_funcdef = False
                return
            if value == "case":
                self.begin_statement_if_needed()
                self.case_depth = 1
                self.previous_word = value
                self.pending_funcdef = False
                return
            if value in COMPOUND_OPEN:
                self.begin_statement_if_needed()
                self.frames.append({
                    "kind": "if" if value == "if" else "loop",
                    "cond": value != "for",
                })
                self.previous_word = value
                self.pending_funcdef = False
                return
            if value in ("then", "do"):
                if self.frames:
                    self.frames[-1]["cond"] = False
                self.previous_word = value
                return
            if value == "elif":
                if self.frames:
                    self.frames[-1]["cond"] = True
                self.previous_word = value
                return
            if value == "else":
                self.previous_word = value
                return
            if value in ("fi", "done"):
                if self.frames:
                    self.frames.pop()
                self.previous_word = value
                self.at_command_position = False
                return
            if value == "{":
                self.begin_statement_if_needed()
                frame_kind = "funcdef" if self.pending_funcdef else "group"
                self.pending_funcdef = False
                self.frames.append({"kind": frame_kind, "cond": False})
                self.previous_word = value
                return
            if value == "}":
                if self.frames:
                    self.frames.pop()
                self.previous_word = value
                self.at_command_position = False
                return
            if value == "time" or (value == "-p" and self.previous_word == "time"):
                self.begin_statement_if_needed()
                self.previous_word = "time"
                return
            if FUNCDEF_RE.match(value):
                self.begin_statement_if_needed()
                self.pending_funcdef = True
                self.previous_word = value
                return
        if kind == "PARENGROUP" and self.pending_funcdef:
            # `f () { ... }`: the standalone parens keep the pending funcdef.
            if value.strip("() \t\n") == "":
                self.previous_word = value
                self.at_command_position = True
                return
        # Any other content: an argument or a plain command word.
        self.begin_statement_if_needed()
        self.previous_word = value
        self.pending_funcdef = False
        self.at_command_position = False

    def finish(self):
        while self.frames:
            self.close_inversions(backgrounded=False)
            self.frames.pop()
        self.close_inversions(backgrounded=False)
        final_statement = self.statement
        results = list(self.flagged)
        for inversion in self.candidates:
            if inversion["statement"] != final_statement:
                results.append((inversion["line"], "status discarded"))
        findings = []
        for line, reason in sorted(results):
            snippet = self.lines[line - 1].strip() if line - 1 < len(self.lines) else ""
            findings.append("%s:%d  [%s]  %s"
                            % (self.relpath, line, reason, snippet[:70]))
        return findings


def scan_file(path, relpath):
    with open(path) as handle:
        text = handle.read()
    lines = text.splitlines()
    findings = []
    tokens = list(Lexer(text).tokens())
    index = 0
    total = len(tokens)
    while index < total:
        kind, value, line = tokens[index]
        opener = None
        if kind == "WORD" and value == "@test":
            opener = value
        elif kind == "WORD" and (value in BODY_OPENERS
                                 or (FUNCDEF_RE.match(value) is not None
                                     and value[:-2] in BODY_OPENERS)):
            opener = value
        if opener is None:
            index += 1
            continue
        # Find the body-opening brace on this definition line.
        index += 1
        while index < total and not (tokens[index][0] == "WORD"
                                     and tokens[index][1] == "{"):
            if tokens[index][0] in ("NL", "OP"):
                break  # not a body definition after all
            index += 1
        if index >= total or tokens[index][1] != "{":
            continue
        index += 1
        depth = 1
        analyzer = BodyAnalyzer(relpath, lines)
        while index < total and depth > 0:
            kind, value, line = tokens[index]
            if kind == "WORD" and value == "{" and analyzer.case_depth == 0:
                depth += 1
            elif kind == "WORD" and value == "}" and analyzer.case_depth == 0:
                depth -= 1
                if depth == 0:
                    index += 1
                    break
            analyzer.feed(kind, value, line)
            index += 1
        findings.extend(analyzer.finish())
    return findings


dead = []
for dirpath, _, names in os.walk(scan_root):
    for name in sorted(names):
        if not name.endswith(".bats"):
            continue
        path = os.path.join(dirpath, name)
        relpath = os.path.relpath(path, scan_root)
        try:
            dead.extend(scan_file(path, relpath))
        except SyntaxError as error:
            print("%s: unlexable: %s" % (relpath, error), file=sys.stderr)
            sys.exit(2)

for entry in dead:
    print(entry)
PY
)"

if [[ -n $report ]]; then
  printf 'FAIL: %s bats assertion(s) invert a command where the status cannot fail the test:\n' \
    "$(printf '%s\n' "$report" | wc -l | tr -d ' ')" >&2
  printf '%s\n' "$report" | sed 's/^/  /' >&2
  printf 'An inverted command only decides the test as the LAST command the body executes; backgrounding, a following command, a non-final statement, or an enclosing brace/if/loop compound all discard its status. Wrap it: if cmd; then echo "why this is wrong"; false; fi, or use a refute helper, or add a || handler.\n' >&2
  exit 1
fi

printf 'no-dead-refutation-in-bats: OK (no bats assertion inverts a command where its status is discarded)\n'
