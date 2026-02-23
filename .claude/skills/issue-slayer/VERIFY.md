# Verification Quality Gate

Run all four commands in the worktree before pushing. All must pass.

```bash
cargo fmt --all -- --check
cargo clippy --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

## On failure

- `cargo fmt --check` fails → run `cargo fmt --all` and re-check
- `cargo clippy` fails → fix all warnings (clippy runs with `-D warnings`)
- `cargo test` fails → fix failing tests before proceeding
- `cargo build --release` fails → fix compile errors

Do not push on any failure.

## Manual test command

After all four pass, output this block so the user can launch the app for
visual testing (replace the path with the actual worktree absolute path):

```
To test manually, run:

pushd <worktree-absolute-path>
$env:RUST_LOG="warn"; cargo run --release -- example.sldshow
```

Example path: `D:\git\sldshow2\.agent-worktrees\feat-issue-42-ambient-fit-shader`
