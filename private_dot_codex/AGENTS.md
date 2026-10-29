## Rules

- Delete with `trash`, never `rm`.
- Make git worktrees with `wt switch --create <branch>`. After merging a PR, run `wt step prune`.

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

## Git and GitHub

- Worktrees and branches:
  - Use `wt switch --create <branch>` to create a branch connected to a worktree.
  - Isolate pieces of work within their own worktree. Unrelated work should be done in separate worktrees.
- Commits:
  - One commit per behavior change. History reads as what changed, never as how the work went.
  - Commit a behavior change once it is done.
  - Feedback on a commit that main does not hold yet, including an edit the owner makes by hand, is folded into that commit, never into a new one:
    - Use `git commit --amend` for the latest commit.
    - Use `git commit --fixup <commit>` and `git rebase --autosquash` for an older commit.
    - If the branch has already been pushed, then use `git push --force-with-lease` to push the updates.
  - Write a commit body only when it adds to the subject line. Keep it short.
  - The body holds only the why and what the diff does not show. No trailers, links or sign-offs that name the tool, model or session that wrote the commit.
- Pull Requests:
  - Opening a PR, merging it, and setting auto-merge are three separate requests. Do one only when the owner asks for it by name. A request to open a PR is not a request to merge it.
  - Opening: one PR per piece of work. Unrelated work gets its own PR.
  - Fixing: fix an open PR by editing or renaming it, and keep the same PR.
  - Merging:
    1. Use `wt switch main` to move to the `main` branch checkout.
    2. Then use `npx -y gh-axi pr merge <number> --rebase` to merge the PR. GitHub deletes the remote branch.
    3. Then use `wt remove <branch>` to remove the worktree and the local branch, and close the herdr panes that sat in it.
    4. Then check `wt list`. If the branch or its worktree is still listed, report what was left behind and why.

## Subagent Delegation

- Delegate all ElevenLabs, video, animation and motion-graphics work to the lara subagent.
