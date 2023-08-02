# Simperby Development Guide

## Checklist

- run `cargo fmt`
- run `cargo clippy --all --all-targets --all-features`
- run `cargo test --all --all-targets --all-features`
- run `./gendoc.sh` only if [cli.rs](../cli/src/cli.rs) is modified

## Commit Message Convention

```text
<title>
```

or

```text
<title>

<description>
```

- `<title>`
  - summarize your change
  - use imperative mood.
  - Omit articles (`a/an` and `the`).
  - should not exceed 72 characters
  - must be capitalized
- `<description>`
  - detailed explanation of your change
  - should not exceed 72 characters

### Examples

```text
Add account check logic

We have to verify 3 things here:
1. blah
2. blah
3. blah
```

### How to set commit message template

- `git config --global commit.template .gitmessage`

## Pull Request Convention

- For the title, use imperative mood and capitalize it.
- Use **rebase only**.
- **NO MERGE COMMITS**.
- Push your works on your own forked repo, and make a **PR across forks**.
- Use the same title (head) and content (body) for a single-commit PR.
- When the PR resolves some issues, put `fixes #123` in the content of the PR,
  not in the commit messages.
- Conversation **must be resolved by the commentor**, not the PR author.
- Do not rely on Github notifications; directly mention the relevant people in
  Discord instead.
- To resolve reviews, put additional commits representing the changes, not amend
  the existing commits and force push it.
- If such commits are to be squashed, put the prefix `fixup!` in the commit
  title.
- However, if it is about commit messages or trivial errors, it's ok to amend
  the existing commits and force push it.
- After all reviews are resolved, the PR author may squash and organize the
  commits, and force push it upon request.

## Rust

### Use

Use a single chunk of `use`.

```rust
use a;
use b;
use c;
```

not

```rust
use a;
use b;

use c;
```

### `Rc` & `Arc`

Prefer using `Rc::clone(&object)` over `object.clone()`:

```rust
let object = std::sync::Rc::new(inner_object);
// Explicit cloning of `Rc`.
let another_object1 = std::sync::Rc::clone(&object);
// This can be seen as cloning the inner object, not cloning the `Rc`, by other reviewers.
let another_object2 = object.clone();
```
