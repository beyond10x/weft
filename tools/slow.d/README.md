# The slow lane

`task slow` runs every executable file of this directory in name order (`tools/run-hooks.sh`);
the first that fails fails the lane. It is never part of `task check`.

Contract of a hook:

- Runs from the repository root, without arguments.
- Regenerates a product from its sources afresh and compares the bytes with the product the gate
  uses: a committed file (the conformance suite), or a cache keyed by the digest of its sources
  (the generated code in `target/generated`, `tools/synth.sh`). It fails on any difference and
  changes no committed file.
- Runs only when the sources changed (`~/beyond10x/AGENTS.md` "Tests stay fast").
- Writes its files under `target/` and is idempotent.
- POSIX sh or a Rust binary; no Python.
