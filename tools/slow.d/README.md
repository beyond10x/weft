# The slow lane

`task slow` runs every executable file of this directory in name order (`tools/run-hooks.sh`);
the first that fails fails the lane. It is never part of `task check`.

Contract of a hook:

- Runs from the repository root, without arguments.
- Regenerates a committed product from its sources (the generated code, the conformance suite) and
  compares the bytes with the committed files; it fails on any difference and changes no
  committed file.
- Runs only when the sources changed (`~/beyond10x/AGENTS.md` "Tests stay fast").
- Writes its files under `target/` and is idempotent.
- POSIX sh or a Rust binary; no Python.
