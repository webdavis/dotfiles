---
name: base
description: Load the language-neutral clean-code method from ~/.agents/skills/clean-code and follow it, routing to the Rust or Swift bindings when the argument names one. Use when the user says /clean-code:base or asks for the clean-code method, or when restructuring a tool this repository owns into layered modules, deciding a boundary or where a seam goes, designing a versioned protocol between a tool and its callers, choosing between a database and the filesystem for durable state, planning the pull-request ladder for a large refactor, or reviewing work against SOLID, file-size and test-quality standards.
---

# Clean code: base

`$ARGUMENTS` is the target language. Pick the branch that matches it before doing anything else:

- **`rust`**: load the Rust derivative instead. Read `~/.agents/skills/clean-code-rust/SKILL.md`,
  which starts by sending you to the method.
- **`swift`**: load the Swift derivative instead. Read `~/.agents/skills/clean-code-swift/SKILL.md`,
  same shape.
- **empty**: ask the user one question, which language this is for, offering rust, swift or other, and
  wait for the answer before reading anything.
- **`other`, or any other language name**: read `~/.agents/skills/clean-code/SKILL.md` and apply the
  method with your own knowledge of that language and whatever language server is attached to the
  project. There is no derivative for it, so say in your first reply which language you assumed and
  what stands in for the derivative's gates.

Whichever branch you take, follow the store copy exactly and never restate, summarize or paraphrase
it here: it is the canonical standard for every harness, and it is the only statement of it. Every
relative link inside those files resolves in the store directory that holds it.

If the store copy the branch names is not there, say so and stop. Do not work the standard from
memory.
