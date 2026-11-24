export type Word = {
  text: string;
  isLiteral: boolean;
};

export type SimpleCommand = {
  assignments: Word[];
  program?: Word;
  args: Word[];
  hereDocuments: string[];
};

export function simpleCommandsIn(script: string): SimpleCommand[] {
  const commands: SimpleCommand[] = [];
  new Reader(script, commands).readAll();
  return commands;
}

type NextWord = "argument" | "redirect-target" | "here-string" | "heredoc-delimiter";
type PendingHeredoc = { delimiter: string; stripsTabs: boolean; expands: boolean; hereDocuments: string[] };

const BLANKS = " \t";
const COMMAND_SEPARATORS = ";&|()";
const GLOB_CHARACTERS = "*?[";
const DOUBLE_QUOTE_ESCAPES = '$`"\\';
const RESERVED_WORDS = ["{", "}", "!", "if", "then", "else", "elif", "fi", "do", "done", "while", "until", "time"];
const ASSIGNMENT = /^[A-Za-z_][A-Za-z0-9_]*\+?=/;

class Reader {
  #position = 0;
  #words: Word[] = [];
  #hereDocuments: string[] = [];
  #text = "";
  #isLiteral = true;
  #isQuoted = false;
  #inWord = false;
  #nextWord: NextWord = "argument";
  #heredocStripsTabs = false;
  #pendingHeredocs: PendingHeredoc[] = [];

  constructor(
    private readonly script: string,
    private readonly commands: SimpleCommand[],
  ) {}

  readAll() {
    while (this.#position < this.script.length) this.#readNext();
    this.#endCommand();
  }

  #readNext() {
    const character = this.script[this.#position];
    const following = this.script[this.#position + 1];
    if (BLANKS.includes(character)) {
      this.#endWord();
      this.#position++;
    } else if (character === "\n") {
      this.#endCommand();
      this.#position++;
      this.#readHeredocBodies();
    } else if (character === "#" && !this.#inWord) {
      const lineEnd = this.script.indexOf("\n", this.#position);
      this.#position = lineEnd < 0 ? this.script.length : lineEnd;
    } else if (character === "\\") {
      if (following !== "\n") this.#append(following ?? "");
      this.#position += 2;
    } else if (character === "'") {
      this.#readSingleQuoted();
    } else if (character === '"') {
      this.#readDoubleQuoted();
    } else if (character === "`") {
      this.#readBackticks();
    } else if (character === "$") {
      this.#readDollar();
    } else if (character === "<" || character === ">" || (character === "&" && following === ">")) {
      this.#readRedirection();
    } else if (COMMAND_SEPARATORS.includes(character)) {
      this.#endCommand();
      this.#position++;
    } else {
      if (GLOB_CHARACTERS.includes(character)) this.#isLiteral = false;
      this.#append(character);
      this.#position++;
    }
  }

  #append(text: string) {
    this.#text += text;
    this.#inWord = true;
  }

  #appendExpansion(source: string) {
    this.#append(source);
    this.#isLiteral = false;
  }

  #readSingleQuoted() {
    const close = this.script.indexOf("'", this.#position + 1);
    const end = close < 0 ? this.script.length : close;
    this.#append(this.script.slice(this.#position + 1, end));
    this.#isQuoted = true;
    this.#position = end + 1;
  }

  #readDoubleQuoted() {
    this.#position++;
    this.#append("");
    this.#isQuoted = true;
    while (this.#position < this.script.length && this.script[this.#position] !== '"') {
      const character = this.script[this.#position];
      const following = this.script[this.#position + 1];
      if (character === "\\" && following === "\n") {
        this.#position += 2;
      } else if (character === "\\" && following !== undefined && DOUBLE_QUOTE_ESCAPES.includes(following)) {
        this.#append(following);
        this.#position += 2;
      } else if (character === "$") {
        this.#readDollar();
      } else if (character === "`") {
        this.#readBackticks();
      } else {
        this.#append(character);
        this.#position++;
      }
    }
    this.#position++;
  }

  #readBackticks() {
    let end = this.#position + 1;
    while (end < this.script.length && this.script[end] !== "`") end += this.script[end] === "\\" ? 2 : 1;
    const inner = this.script.slice(this.#position + 1, end).replace(/\\([`$\\])/g, "$1");
    this.commands.push(...simpleCommandsIn(inner));
    this.#appendExpansion("");
    this.#position = end + 1;
  }

  #readDollar() {
    const start = this.#position;
    const following = this.script[start + 1];
    if (following === "(" && this.script[start + 2] === "(") {
      this.#position = closingParenthesis(this.script, start + 1) + 1;
      this.#appendExpansion(this.script.slice(start, this.#position));
    } else if (following === "(") {
      const close = closingParenthesis(this.script, start + 1);
      this.commands.push(...simpleCommandsIn(this.script.slice(start + 2, close)));
      this.#position = close + 1;
      this.#appendExpansion("");
    } else if (following === "{") {
      const close = this.script.indexOf("}", start + 2);
      this.#position = close < 0 ? this.script.length : close + 1;
      this.#appendExpansion(this.script.slice(start, this.#position));
    } else if (following === "'") {
      let end = start + 2;
      while (end < this.script.length && this.script[end] !== "'") end += this.script[end] === "\\" ? 2 : 1;
      this.#position = end + 1;
      this.#appendExpansion(this.script.slice(start, this.#position));
    } else if (following !== undefined && /[A-Za-z0-9_@*#?$!-]/.test(following)) {
      const name = /^[A-Za-z_][A-Za-z0-9_]*/.exec(this.script.slice(start + 1));
      this.#position = start + 1 + (name ? name[0].length : 1);
      this.#appendExpansion(this.script.slice(start, this.#position));
    } else {
      this.#append("$");
      this.#position++;
    }
  }

  #readRedirection() {
    if (/^\d+$/.test(this.#text) && this.#isLiteral && !this.#isQuoted) this.#resetWord();
    this.#endWord();
    const rest = this.script.slice(this.#position);
    const processSubstitution = /^[<>]\(/.exec(rest);
    if (processSubstitution) {
      const close = closingParenthesis(this.script, this.#position + 1);
      this.commands.push(...simpleCommandsIn(this.script.slice(this.#position + 2, close)));
      this.#position = close + 1;
      this.#appendExpansion("");
      return;
    }
    const operator = /^(<<<|<<-|<<|&>>|&>|[<>]&|<>|>>|>\||[<>])/.exec(rest)?.[0] ?? rest[0];
    this.#position += operator.length;
    if (operator === "<<<") this.#nextWord = "here-string";
    else if (operator.startsWith("<<")) {
      this.#nextWord = "heredoc-delimiter";
      this.#heredocStripsTabs = operator === "<<-";
    } else this.#nextWord = "redirect-target";
  }

  #endWord() {
    if (!this.#inWord) return;
    const word = { text: this.#text, isLiteral: this.#isLiteral };
    if (this.#nextWord === "argument") this.#words.push(word);
    else if (this.#nextWord === "here-string") this.#hereDocuments.push(word.text);
    else if (this.#nextWord === "heredoc-delimiter") {
      this.#pendingHeredocs.push({
        delimiter: word.text,
        stripsTabs: this.#heredocStripsTabs,
        expands: !this.#isQuoted,
        hereDocuments: this.#hereDocuments,
      });
    }
    this.#nextWord = "argument";
    this.#resetWord();
  }

  #resetWord() {
    this.#text = "";
    this.#isLiteral = true;
    this.#isQuoted = false;
    this.#inWord = false;
  }

  #endCommand() {
    this.#endWord();
    if (
      this.#words.length > 0 ||
      this.#hereDocuments.length > 0 ||
      this.#pendingHeredocs.some((heredoc) => heredoc.hereDocuments === this.#hereDocuments)
    ) {
      const assignments: Word[] = [];
      let start = 0;
      for (; start < this.#words.length; start++) {
        const word = this.#words[start];
        if (ASSIGNMENT.test(word.text)) assignments.push(word);
        else if (!RESERVED_WORDS.includes(word.text)) break;
      }
      this.commands.push({
        assignments,
        program: this.#words[start],
        args: this.#words.slice(start + 1),
        hereDocuments: this.#hereDocuments,
      });
    }
    this.#words = [];
    this.#hereDocuments = [];
    this.#nextWord = "argument";
  }

  #readHeredocBodies() {
    for (const heredoc of this.#pendingHeredocs.splice(0)) {
      const lines: string[] = [];
      while (this.#position < this.script.length) {
        const lineEnd = this.script.indexOf("\n", this.#position);
        const end = lineEnd < 0 ? this.script.length : lineEnd;
        const line = this.script.slice(this.#position, end);
        this.#position = end + 1;
        const content = heredoc.stripsTabs ? line.replace(/^\t+/, "") : line;
        if (content === heredoc.delimiter) break;
        lines.push(content);
      }
      const body = lines.join("\n");
      heredoc.hereDocuments.push(body);
      if (heredoc.expands && /\$\(|`/.test(body)) this.commands.push(...simpleCommandsIn(body));
    }
  }
}

function closingParenthesis(script: string, open: number): number {
  let depth = 0;
  for (let index = open; index < script.length; index++) {
    const character = script[index];
    if (character === "\\") index++;
    else if (character === "'" || character === '"' || character === "`") {
      const close = script.indexOf(character, index + 1);
      if (close < 0) return script.length;
      index = close;
    } else if (character === "(") depth++;
    else if (character === ")" && --depth === 0) return index;
  }
  return script.length;
}
