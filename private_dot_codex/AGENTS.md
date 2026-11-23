## Rules

- Delete with `trash`, never `rm`.

## Code

- Code is self-documenting. A comment is the last resort.
- One feature per component. Components talk only through their boundaries. Follow SOLID.
- Keep files small. Split a file before it holds a second feature.
- Code is test-driven: write the failing test first, against the public interface at the feature boundary. Shell scripts and config files are exempt.
- Tests live in their own files, never beside production code.

## Comments

- Comment only what a developer cannot read from the code or setting name. Delete a comment that restates it.
- Say the effect, then any condition it needs, in full sentences. Use a second line only to keep a fact.
- No labels, banners, links, parentheticals, mechanism, or values that drift such as versions and dates. Explain a workaround in words.

Load the `git` skill before working in a git repository.
