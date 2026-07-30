#!/usr/bin/env bash
# no-dead-refutation-in-bats.sh, a REPO-WIDE guard.
#
# THE PROPERTY: inside a bats-executed body (@test, setup, teardown, and their
# file/suite variants), the exit status of a bare inverted command must REACH
# something that can act on it -- the body's final status, errexit, or a branch
# or handler that the status selects. bash's `set -e` and bats' ERR trap both
# ignore a `!` pipeline, and bats takes the body's LAST status as the result,
# so an inverted command that is neither last nor consumed reads like a working
# check and can never fail the test.
#
# The guard therefore has TWO obligations, and the second is worthless without
# the first:
#
#   1. COVERAGE. Every body bats executes must actually be analyzed. Any file,
#      directory, definition spelling or region this scan cannot handle is
#      reported loudly (exit 2), never skipped into a green pass.
#   2. JUDGEMENT. Within an analyzed body, an inversion is left alone only when
#      its status genuinely reaches a consumer.
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
#   { ! cmd; } / if,loop bodies non-final statement    DEAD  (the exemption
#                                                      propagates through
#                                                      non-subshell compounds)
#   time ! cmd                  non-final statement    DEAD  (time is pipeline
#                                                      syntax)
#   if ! cmd; other; then       non-final in the       DEAD  (only the LAST
#                               condition list               command of the list
#                                                      decides the compound)
#   ( ! cmd ) / x=$(! cmd)      anywhere               LIVE  (a subshell or
#                                                      substitution surfaces the
#                                                      status to errexit)
#   f() { ! cmd; }; f           anywhere               LIVE  (the call is a plain
#                                                      command)
#   [[ ! -e x ]]                anywhere               LIVE  (fails via the [[
#                                                      compound, which errexit
#                                                      sees)
#   ! cmd || handler            anywhere               THE HANDLER DECIDES:
#                                                      `|| echo why` passed,
#                                                      `|| { echo why; false; }`
#                                                      failed. So a `||` handler
#                                                      does NOT make a refutation
#                                                      live by itself.
#   if ! cmd; then h; fi        anywhere               THE BRANCH DECIDES, for
#   while ! cmd; do h; done                            the same reason: `then
#                                                      echo why` passed.
#
# So the guard flags an inverted pipeline when its status is discarded: it is
# backgrounded, or it is a non-final command of an if/while/until condition
# list, or it sits outside the body's final top-level statement with nothing
# consuming it (no condition, no following `||` handler, no enclosing subshell
# or substitution, not a function-definition body).
#
# CLAIMED EXACTLY, NO MORE. Two kinds of limit, deliberately separated because
# they behave differently and were once described as if they were the same:
#
# (a) PRESUMED LIVE -- input that is dead in reality and passes anyway. Each is
#     pinned as a boundary fixture in test/test-system/dead-refutation-shapes.sh
#     and the identifier in brackets is what ties the two lists together:
#   - [final-compound] inside the body's FINAL compound statement (e.g.
#     `{ ! cmd; true; }` as the last statement) the inversion is presumed live,
#     because which inner command runs last is data-dependent;
#   - [case-body] case...esac bodies are scanned opaquely, so a dead inversion
#     in a case branch passes;
#   - [parenthesized] parenthesized contexts are presumed consumed, so a dead
#     inversion in a process substitution passes;
#   - [function-body] function BODIES are exempt (calls are live; a
#     multi-command body hiding a dead inversion passes);
#   - [file-scope-helper] file-scope helper functions other than the bats
#     setup/teardown family are not scanned at all;
#   - [or-handler] an inversion whose status is consumed by a following `||`
#     is presumed live, because the handler MAY fail; a handler that cannot
#     fail (`|| echo why`) leaves the refutation dead;
#   - [condition-consumer] an inversion that is the FINAL command of an
#     if/while/until condition is presumed live for the same reason: the branch
#     it selects may or may not fail;
#   - [unscanned-suffix] only the suffixes in SCANNED_FILE_SUFFIXES are read,
#     so a bats body in a helper `load`ed under another suffix is not seen.
#
# (b) REFUSED -- input this scan cannot read correctly, reported with exit 2 and
#     a diagnostic naming the file, never a green pass. The list is exhaustive
#     on purpose: every refusal the code can raise is named here and pinned by
#     its own fixture, because a refusal set that is documented in part makes
#     the "these lists describe one contract" diff a partial one:
#   - [heredoc-in-substitution] a heredoc body inside a command substitution is
#     swallowed as substitution text, so an apostrophe or an unbalanced paren
#     in it aborts the scan;
#   - [unterminated-quote] a quote the lexer never sees closed, which would
#     otherwise swallow the rest of the file as quoted text;
#   - [unbalanced-parens] a $(...), ${...}, <(...) or [[ ... ]] the lexer never
#     sees closed, for the same reason;
#   - [unclosed-body] a bats-executed body whose braces never balance, which
#     would otherwise swallow every body after it;
#   - [unterminated-case] a case...esac opened inside a bats-executed body and
#     never closed, which would otherwise freeze the region tracker;
#   - [unbalanced-compound] a bats-executed body whose braces balance while an
#     if/loop/group frame is still open, so the two bracket models disagree;
#   - [unreadable-file] a source file that cannot be opened;
#   - [non-utf8-source] a source file that is not valid UTF-8, so its text
#     cannot be read at all;
#   - [unlistable-directory] a directory the walk cannot list, which would
#     otherwise turn a tree holding a dead refutation into a green pass.
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

# The advice printed with a rejection, in three separately named lines because
# the three say different things and a test has to be able to tell them apart.
# The mechanism explains why the status vanished; the RECOMMENDATION lists the
# spellings that were measured to fail when the refutation is violated; the
# WARNING names the near-miss spelling that reads like a fix and cannot fail.
# Keeping them on one line let a swap of the last two keep every substring of
# the original, so a message that recommended the dead `|| echo` spelling and
# warned about the live one still satisfied its pin. The line split is what
# gives test/test-system/dead-refutation-shapes.sh a region to check.
ADVICE_MECHANISM_LINE='An inverted command only decides the test as the LAST command the body executes; backgrounding, a following command, a non-final statement, a non-final command of an if/while condition, or an enclosing brace/if/loop compound all discard its status.'
ADVICE_RECOMMENDATION_LINE='Give the status somewhere to go, and make that somewhere FAIL: if cmd; then echo "why this is wrong"; false; fi, or call a single-command refute helper, or add a handler that fails: || { echo "why this is wrong"; false; }.'
ADVICE_WARNING_LINE='A bare || echo handler cannot fail the test: it reports the violation and returns success.'

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -d $SCAN_ROOT ]] || fail "no such scan root: $SCAN_ROOT"

report="$(
  python3 - "$SCAN_ROOT" <<'PY'
import os, re, sys

scan_root = sys.argv[1]

# --------------------------------------------------------------- constants

# bats runs these function bodies around every test, so a dead refutation in
# one is the same defect as a dead refutation in a @test body.
BATS_BODY_FUNCTION_NAMES = frozenset((
    "setup", "teardown", "setup_file", "teardown_file",
    "setup_suite", "teardown_suite",
))

# bats' preprocessor recognizes TWO test-declaration syntaxes (read from
# bats-core 1.11.1, libexec/bats-core/bats-preprocess): BATS_TEST_PATTERN for
# `@test <description> {` and BATS_TEST_PATTERN_COMMENT for `name() { # @test`.
# Both anchor the opening brace to the declaration LINE, which is why the brace
# search never crosses a newline for them. The comment form is invisible to the
# lexer (comments are stripped), so it is matched against the raw line here,
# with bats' own regex transcribed ([[:blank:]] is space and tab).
BATS_TEST_KEYWORD = "@test"
BATS_COMMENT_TEST_LINE_RE = re.compile(
    r"[ \t]*[^ \t()]+[ \t]*\(?\)?[ \t]+\{[ \t]+#[ \t]*@test[ \t]*$")

# bats' `load` appends .bash when the given path does not exist, and
# setup_suite/teardown_suite live in setup_suite.bash by design, so a bats body
# is not confined to a .bats file. Other suffixes are limit [unscanned-suffix].
SCANNED_FILE_SUFFIXES = (".bats", ".bash")

# Compound commands whose body the analyzer frames. The value says whether the
# words between the opener and `then`/`do` are a CONDITION list, whose LAST
# command is the one the compound consumes.
COMPOUND_OPENERS = {
    "if": True, "while": True, "until": True, "for": False, "select": False,
}
CONDITION_CLOSERS = frozenset(("then", "do"))
FUNCTION_KEYWORD = "function"

# `name()` arrives glued into one word by the lexer; `name ()` and
# `function name` arrive as a bare name.
#
# A definition is recognized by its SHAPE, not by guessing which names are
# legal. bash's function names are far wider than a POSIX identifier
# (`refute-x`, `refute.x`, `refute:x`, `2fa`, `a+b`, `a[b]` and `::` all define
# functions, measured with `bash -c '<name>() { :; }'`), and every name the
# scan fails to recognize costs twice: the definition's `{` stops being read as
# an opener, so the helper's body is analyzed as ordinary code and its
# inversion reported dead while the call that consumes it is live. Matching on
# shape has no such gap, because bash has no construct other than a function
# definition in which a word ends in `()`. The one word that does and is NOT a
# definition is an empty array assignment (`x=()`), and `=` is exactly what
# bash rejects inside a function name, so an `=` disqualifies the word and the
# brace group after an assignment stays an ordinary group.
ASSIGNMENT_CHARACTER = "="
GLUED_FUNCTION_DEFINITION_RE = re.compile(
    r"^[^%s]+\(\)$" % ASSIGNMENT_CHARACTER)
FUNCTION_NAME_RE = re.compile(r"^[^%s]+$" % ASSIGNMENT_CHARACTER)

REASON_BACKGROUNDED = "backgrounded"
REASON_DISCARDED = "status discarded"
REASON_DISCARDED_IN_CONDITION = "discarded in condition list"


class UnanalyzableSource(Exception):
    """This file's scan cannot be trusted, so it must never be reported clean."""


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
        raise UnanalyzableSource("line %d: %s" % (self.line, message))

    def take(self):
        ch = self.text[self.i]
        self.i += 1
        if ch == "\n":
            self.line += 1
        return ch

    def peek(self, k=0):
        j = self.i + k
        return self.text[j] if j < self.n else ""

    def peek_is(self, characters, k=0):
        """Is the k-th unread character one of these? End of input is NOT: the
        empty string is a substring of every string, so a bare
        `self.peek() in "..."` is True at end of input and the take() behind it
        raises IndexError on any file that stops mid-token (no trailing newline
        is enough)."""
        ch = self.peek(k)
        return ch != "" and ch in characters

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
            elif ch == "$" and self.peek_is("({"):
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
            elif ch == "$" and self.peek_is("({'"):
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
            elif ch == "$" and self.peek_is("({"):
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
        while self.peek_is(" \t"):
            self.take()
        delimiter = ""
        while self.i < self.n and not self.peek_is(" \t\n;&|<>()"):
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
                while self.peek_is(";&"):  # ;; ;& ;;& all end a list
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
                if self.peek_is("|&"):
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
                if self.peek_is(">&"):
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
                # An unmatched closer: a case-branch pattern terminator (every
                # other paren context is swallowed above). Emitted rather than
                # dropped, because it is what tells the case tracker that a
                # branch BODY has started.
                token = flush()
                if token:
                    yield token
                line = self.line
                self.take()
                yield ("OP", ")", line)
                continue
            # A plain word character.
            if word_line is None:
                word_line = self.line
            word += self.take()
            # A completed [[ word triggers an opaque swallow to ]].
            if word == "[[" and self.peek_is(" \t\n"):
                line = word_line
                self.skip_dbrackets()
                yield ("WORD", "[[...]]", line)
                word = ""
                word_line = None
        token = flush()
        if token:
            yield token


class CaseRegionTracker:
    """Follow a case...esac region without analyzing it (limit [case-body]).

    Tracked as a state machine rather than by counting bare `case`/`esac`
    words, because a word count cannot tell a KEYWORD from an ARGUMENT: one
    `grep -q case file` inside a branch would raise the depth forever, and the
    rest of the file would be swallowed with no diagnostic. States mirror
    bash's grammar, so only a keyword in keyword position moves the region."""

    AWAITING_IN, PATTERN, BODY = "awaiting-in", "pattern", "body"
    BRANCH_TERMINATORS = frozenset((";;", ";&", ";;&"))

    def __init__(self):
        self.states = []

    @property
    def depth(self):
        return len(self.states)

    def open_region(self):
        self.states.append(self.AWAITING_IN)

    def feed(self, kind, value, at_command_position):
        """Advance the region. Returns True when the OUTERMOST case closed."""
        state = self.states[-1]
        if state == self.AWAITING_IN:
            if kind == "WORD" and value == "in":
                self.states[-1] = self.PATTERN
            return False
        if state == self.PATTERN:
            # `esac` right after `in` or after a branch terminator ends the
            # region; a `)` or a parenthesized `(a|b)` pattern starts a body.
            if kind == "WORD" and value == "esac":
                self.states.pop()
                return not self.states
            if (kind == "OP" and value == ")") or kind == "PARENGROUP":
                self.states[-1] = self.BODY
            return False
        if kind == "OP" and value in self.BRANCH_TERMINATORS:
            self.states[-1] = self.PATTERN
        elif kind == "WORD" and at_command_position:
            if value == "case":
                self.open_region()
            elif value == "esac":
                self.states.pop()
                return not self.states
        return False


class BodyAnalyzer:
    """Positional analysis of one bats-executed body's token stream."""

    CONTINUATION_OPS = frozenset(("&&", "||", "|", "|&"))

    def __init__(self, relpath, lines):
        self.relpath = relpath
        self.lines = lines
        self.frames = []      # {"kind": group|funcdef|if|loop, "cond": bool, ...}
        self.statement = 0    # top-level statement counter
        self.need_new_statement = True
        self.at_command_position = True
        self.case_region = CaseRegionTracker()
        self.open_inversions = []       # sites not yet terminated
        self.candidates = []            # closed, pending the final-statement check
        self.condition_candidates = []  # closed, pending the condition-list check
        self.flagged = []               # (line, reason) definite findings
        self.pending_funcdef = False
        self.awaiting_function_name = False
        self.candidate_function_name = None
        self.previous_word = None
        self.last_significant = None

    # ---------------------------------------------------------- predicates

    @property
    def case_depth(self):
        return self.case_region.depth

    def in_condition(self):
        return bool(self.frames) and self.frames[-1].get("cond", False)

    def in_funcdef(self):
        return any(frame["kind"] == "funcdef" for frame in self.frames)

    def current_condition_group(self):
        return self.frames[-1]["condition_group"] if self.in_condition() else None

    # ------------------------------------------------------ site lifecycle

    def flag(self, line, reason):
        self.flagged.append((line, reason))

    def open_inversion(self, line):
        self.open_inversions.append({
            "line": line,
            "level": len(self.frames),
            "statement": self.statement,
            "or_after": False,
            "condition_group": self.current_condition_group(),
            "funcdef": self.in_funcdef(),
        })

    def close_inversions(self, backgrounded):
        level = len(self.frames)
        remaining = []
        for inversion in self.open_inversions:
            if inversion["level"] != level:
                remaining.append(inversion)
                continue
            if inversion["funcdef"]:
                continue
            if backgrounded:
                self.flag(inversion["line"], REASON_BACKGROUNDED)
            elif inversion["condition_group"] is not None:
                # Only the LAST command of the condition list decides the
                # compound; which one that is is known at `then`/`do`.
                self.condition_candidates.append(inversion)
            elif inversion["or_after"]:
                pass  # limit [or-handler]: the handler may fail
            else:
                self.candidates.append(inversion)
        self.open_inversions = remaining

    def resolve_condition_candidates(self):
        """A condition list has ended. Its final command is what the compound
        consumes (limit [condition-consumer]); every earlier command's status is
        discarded, so an inversion there is dead."""
        if not self.frames:
            return
        final_group = self.frames[-1]["condition_group"]
        level = len(self.frames)
        remaining = []
        for inversion in self.condition_candidates:
            if inversion["level"] != level:
                remaining.append(inversion)
            elif inversion["condition_group"] != final_group:
                self.flag(inversion["line"], REASON_DISCARDED_IN_CONDITION)
        self.condition_candidates = remaining

    # ------------------------------------------------------- frame handling

    def push_frame(self, kind, cond):
        self.frames.append({
            "kind": kind,
            "cond": cond,
            "condition_group": 0,
            "condition_needs_new_group": True,
        })

    def pop_frame(self):
        """Close and resolve everything the frame still owns BEFORE dropping it,
        the close-then-pop order the end-of-body sweep has always used. A site
        left open across a pop would never match a frame level again and would
        vanish from the report. No fixture reaches that: bash requires a list
        terminator before `}`, `fi` and `done`, so in valid input every site is
        already closed here. This is the net for input that is not."""
        self.close_inversions(backgrounded=False)
        self.resolve_condition_candidates()
        if self.frames:
            self.frames.pop()

    def begin_command_group_if_needed(self):
        """A command word starts a new top-level statement, or a new command in
        the enclosing condition list. Both are 'which group am I in' counters
        and both are bumped lazily, so a trailing separator before `then` or
        before the body's end does not invent an empty final group."""
        if not self.frames:
            if self.need_new_statement:
                self.statement += 1
                self.need_new_statement = False
            return
        frame = self.frames[-1]
        if frame["cond"] and frame["condition_needs_new_group"]:
            frame["condition_group"] += 1
            frame["condition_needs_new_group"] = False

    def end_command_group(self):
        if not self.frames:
            self.need_new_statement = True
        elif self.frames[-1]["cond"]:
            self.frames[-1]["condition_needs_new_group"] = True

    def forget_pending_definition(self):
        self.awaiting_function_name = False
        self.candidate_function_name = None

    # --------------------------------------------------------------- feed

    def feed(self, kind, value, line):
        if self.case_region.depth:
            closed = self.case_region.feed(kind, value, self.at_command_position)
            self.at_command_position = kind in ("NL", "OP")
            if closed:
                self.at_command_position = False
                self.last_significant = value
            return
        if kind == "NL":
            self.candidate_function_name = None
            if self.last_significant in self.CONTINUATION_OPS:
                return  # the list continues on the next line
            self.close_inversions(backgrounded=False)
            self.end_command_group()
            self.at_command_position = True
            return
        if kind == "OP":
            self.last_significant = value
            self.previous_word = None
            self.pending_funcdef = False
            self.forget_pending_definition()
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
            else:  # ; ;; ;& ;;& & )
                backgrounded = value == "&"
                self.close_inversions(backgrounded)
                self.end_command_group()
            self.at_command_position = True
            return

        # WORD / PARENGROUP tokens.
        was_at_command_position = self.at_command_position
        self.last_significant = value
        if kind == "WORD" and was_at_command_position:
            if self.feed_command_word(value, line):
                return
        if kind == "PARENGROUP" and value.strip("() \t\n") == "":
            # The standalone parens of `name ()` or `function name ()`.
            if self.pending_funcdef or self.candidate_function_name is not None:
                self.pending_funcdef = True
                self.forget_pending_definition()
                self.previous_word = value
                self.at_command_position = True
                return
        # Any other content: an argument or a plain command word.
        self.begin_command_group_if_needed()
        self.previous_word = value
        self.pending_funcdef = False
        self.candidate_function_name = (
            value if was_at_command_position and kind == "WORD"
            and FUNCTION_NAME_RE.match(value) else None)
        self.awaiting_function_name = False
        self.at_command_position = False

    def feed_command_word(self, value, line):
        """Handle a WORD in command position. Returns True when it was handled
        here and feed() must not fall through to the argument path."""
        if value == "!":
            self.begin_command_group_if_needed()
            self.open_inversion(line)
            self.previous_word = "!"
            self.pending_funcdef = False
            self.forget_pending_definition()
            return True
        if self.awaiting_function_name and FUNCTION_NAME_RE.match(value):
            # `function name` and `function name ()` both define a function;
            # command position is kept for the `()` or the body brace.
            self.awaiting_function_name = False
            self.pending_funcdef = True
            self.previous_word = value
            return True
        if value == FUNCTION_KEYWORD:
            self.begin_command_group_if_needed()
            self.awaiting_function_name = True
            self.previous_word = value
            return True
        if value == "case":
            self.begin_command_group_if_needed()
            self.case_region.open_region()
            self.previous_word = value
            self.pending_funcdef = False
            self.forget_pending_definition()
            return True
        if value in COMPOUND_OPENERS:
            self.begin_command_group_if_needed()
            self.push_frame("if" if value == "if" else "loop",
                            COMPOUND_OPENERS[value])
            self.previous_word = value
            self.pending_funcdef = False
            self.forget_pending_definition()
            return True
        if value in CONDITION_CLOSERS:
            self.close_inversions(backgrounded=False)
            self.resolve_condition_candidates()
            if self.frames:
                self.frames[-1]["cond"] = False
            self.previous_word = value
            return True
        if value == "elif":
            if self.frames:
                frame = self.frames[-1]
                frame["cond"] = True
                frame["condition_group"] = 0
                frame["condition_needs_new_group"] = True
            self.previous_word = value
            return True
        if value == "else":
            self.previous_word = value
            return True
        if value in ("fi", "done"):
            self.pop_frame()
            self.previous_word = value
            self.at_command_position = False
            return True
        if value == "{":
            self.begin_command_group_if_needed()
            frame_kind = "funcdef" if self.pending_funcdef else "group"
            self.pending_funcdef = False
            self.forget_pending_definition()
            self.push_frame(frame_kind, False)
            self.previous_word = value
            return True
        if value == "}":
            self.pop_frame()
            self.previous_word = value
            self.at_command_position = False
            return True
        if value == "time" or (value == "-p" and self.previous_word == "time"):
            self.begin_command_group_if_needed()
            self.previous_word = "time"
            return True
        if GLUED_FUNCTION_DEFINITION_RE.match(value):
            self.begin_command_group_if_needed()
            self.pending_funcdef = True
            self.previous_word = value
            self.forget_pending_definition()
            return True
        return False

    def finish(self):
        while self.frames:
            self.pop_frame()
        self.close_inversions(backgrounded=False)
        self.resolve_condition_candidates()
        final_statement = self.statement
        results = list(self.flagged)
        for inversion in self.candidates:
            if inversion["statement"] != final_statement:
                results.append((inversion["line"], REASON_DISCARDED))
        findings = []
        for line, reason in sorted(results):
            snippet = self.lines[line - 1].strip() if line - 1 < len(self.lines) else ""
            findings.append("%s:%d  [%s]  %s"
                            % (self.relpath, line, reason, snippet[:70]))
        return findings


def bats_comment_test_lines(text):
    """Line numbers that bats' preprocessor turns into a test function through
    its comment syntax (`name() { # @test`)."""
    return frozenset(
        number for number, line in enumerate(text.splitlines(), start=1)
        if BATS_COMMENT_TEST_LINE_RE.search(line))


def first_token_on_line_flags(tokens):
    """Per-token: is this the first token on its line? bats anchors both test
    syntaxes to the start of the line, so `@test` must be too."""
    flags = []
    previous_line = None
    for _, _, line in tokens:
        flags.append(line != previous_line)
        previous_line = line
    return flags


def find_brace_on_same_line(tokens, index, line):
    """The index of a `{` token on `line`, scanning forward from index. A
    newline or an operator means this was not a body definition after all."""
    while index < len(tokens):
        kind, value, token_line = tokens[index]
        if kind in ("NL", "OP") or token_line != line:
            return None
        if kind == "WORD" and value == "{":
            return index
        index += 1
    return None


def find_function_body_brace(tokens, index):
    """The index of the `{` opening a bats body function defined at `index`, in
    any spelling bash accepts:

        setup() {        setup () {        function setup {        function setup () {

    and with newlines before the brace, which bash allows for a function
    definition (bats' two test syntaxes, by contrast, need it on the same line).
    A bare `setup {` is NOT a definition: it is the command `setup` with the
    argument `{`, so parens or the `function` keyword are required."""
    total = len(tokens)
    saw_function_keyword = False
    kind, value, _ = tokens[index]
    if kind != "WORD":
        return None
    if value == FUNCTION_KEYWORD:
        saw_function_keyword = True
        index += 1
        if index >= total or tokens[index][0] != "WORD":
            return None
        value = tokens[index][1]
    if GLUED_FUNCTION_DEFINITION_RE.match(value):
        name, saw_parens = value[:-2], True
    else:
        name, saw_parens = value, False
    if name not in BATS_BODY_FUNCTION_NAMES:
        return None
    index += 1
    if (not saw_parens and index < total and tokens[index][0] == "PARENGROUP"
            and tokens[index][1].strip("() \t\n") == ""):
        saw_parens = True
        index += 1
    if not (saw_parens or saw_function_keyword):
        return None
    while index < total and tokens[index][0] == "NL":
        index += 1
    if index < total and tokens[index][0] == "WORD" and tokens[index][1] == "{":
        return index
    return None


def find_body_open_brace(tokens, index, first_on_line, comment_test_lines):
    """The index of the `{` that opens a bats-executed body declared at
    `index`, or None. Discovery only: what is inside the body is
    BodyAnalyzer's job."""
    kind, value, line = tokens[index]
    if kind != "WORD":
        return None
    if value == BATS_TEST_KEYWORD and first_on_line[index]:
        return find_brace_on_same_line(tokens, index + 1, line)
    if value == "{" and line in comment_test_lines:
        return index
    return find_function_body_brace(tokens, index)


def analyze_body(tokens, index, open_line, relpath, lines):
    """Analyze the body whose contents start at `index` (just past its opening
    brace on `open_line`). Returns (findings, index just past its closing
    brace). Any structure the scan cannot resolve is REFUSED rather than
    reported clean: each of them swallows the rest of the file silently.

    `{` and `}` delimit the body only in COMMAND POSITION, which is bash's own
    rule for them as reserved words: `echo }` prints a brace, it does not close
    a group (measured). Counting them anywhere had an asymmetry that failed
    OPEN in the direction that matters -- a `}` in argument position ended the
    body early and every later line, dead refutations included, was silently
    reread as unscanned file scope and reported clean, while a `{` in argument
    position merely refused."""
    analyzer = BodyAnalyzer(relpath, lines)
    depth = 1
    total = len(tokens)
    closed = False
    while index < total:
        kind, value, line = tokens[index]
        if (kind == "WORD" and analyzer.case_depth == 0
                and analyzer.at_command_position):
            if value == "{":
                depth += 1
            elif value == "}":
                depth -= 1
                if depth == 0:
                    index += 1
                    closed = True
                    break
        analyzer.feed(kind, value, line)
        index += 1
    if analyzer.case_depth != 0:
        raise UnanalyzableSource(
            "line %d: a case...esac opened in this bats-executed body is never "
            "closed" % open_line)
    if not closed:
        raise UnanalyzableSource(
            "line %d: this bats-executed body is never closed" % open_line)
    if analyzer.frames:
        # The body's braces balanced while the analyzer still holds an open
        # if/loop/group frame, so the two bracket models disagree and the
        # positional verdicts computed from the frames cannot be trusted.
        # Valid input never reaches this (bash requires fi/done/} first), so it
        # is a desync net, and a desync must refuse rather than report clean.
        raise UnanalyzableSource(
            "line %d: a compound command opened in this bats-executed body is "
            "never closed" % open_line)
    return analyzer.finish(), index


def scan_file(relpath, text):
    lines = text.splitlines()
    comment_test_lines = bats_comment_test_lines(text)
    tokens = list(Lexer(text).tokens())
    first_on_line = first_token_on_line_flags(tokens)
    findings = []
    index = 0
    while index < len(tokens):
        brace = find_body_open_brace(tokens, index, first_on_line,
                                     comment_test_lines)
        if brace is None:
            index += 1
            continue
        body_findings, index = analyze_body(
            tokens, brace + 1, tokens[brace][2], relpath, lines)
        findings.extend(body_findings)
    return findings


def collect_source_files(root):
    """Every file to scan, plus the errors that made the walk incomplete.

    Symlinked directories ARE followed: a symlinked suite directory is still a
    suite directory, and skipping it would hide bodies bats runs. Each
    directory is visited at most once, keyed by (device, inode), so a symlink
    cycle terminates instead of looping. A directory the walk cannot read is an
    ERROR, never a silent skip: os.walk's default swallows it and turns a
    failing tree into a green one."""
    errors = []
    visited_directories = set()

    def visit_once(path):
        try:
            info = os.stat(path)
        except OSError as error:
            errors.append(error)
            return False
        key = (info.st_dev, info.st_ino)
        if key in visited_directories:
            return False
        visited_directories.add(key)
        return True

    visit_once(root)
    found = []
    for dirpath, dirnames, filenames in os.walk(
            root, onerror=errors.append, followlinks=True):
        kept = []
        for name in sorted(dirnames):
            if visit_once(os.path.join(dirpath, name)):
                kept.append(name)
        dirnames[:] = kept
        for name in sorted(filenames):
            if name.endswith(SCANNED_FILE_SUFFIXES):
                path = os.path.join(dirpath, name)
                found.append((path, os.path.relpath(path, root)))
    return found, errors


def refuse(message):
    print(message, file=sys.stderr)
    sys.exit(2)


source_files, walk_errors = collect_source_files(scan_root)
if walk_errors:
    for walk_error in walk_errors:
        print("cannot list %s: %s"
              % (getattr(walk_error, "filename", scan_root), walk_error),
              file=sys.stderr)
    refuse("refusing to report a partial scan of %s" % scan_root)

dead = []
for source_path, source_relpath in source_files:
    try:
        with open(source_path, encoding="utf-8") as handle:
            source_text = handle.read()
    except (OSError, UnicodeDecodeError) as read_error:
        refuse("%s: cannot read: %s" % (source_relpath, read_error))
    try:
        dead.extend(scan_file(source_relpath, source_text))
    except UnanalyzableSource as scan_error:
        refuse("%s: cannot analyze: %s" % (source_relpath, scan_error))

for entry in dead:
    print(entry)
PY
)"

if [[ -n $report ]]; then
  printf 'FAIL: %s bats assertion(s) invert a command where the status cannot fail the test:\n' \
    "$(printf '%s\n' "$report" | wc -l | tr -d ' ')" >&2
  printf '%s\n' "$report" | sed 's/^/  /' >&2
  printf '%s\n%s\n%s\n' \
    "$ADVICE_MECHANISM_LINE" "$ADVICE_RECOMMENDATION_LINE" "$ADVICE_WARNING_LINE" >&2
  exit 1
fi

printf 'no-dead-refutation-in-bats: OK (no bats assertion inverts a command where its status is discarded)\n'
