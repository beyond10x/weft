# Further gate steps

`task check` runs every executable file of this directory in name order (`tools/run-hooks.sh`)
after the tests (task `test`) and the gate self-test (task `gate-budget`); the first that fails
fails the gate. The CI Gate job reaches them through `task check`. Examples: the digest check of
the generated code, the conformance run.

Contract of a hook:

- Runs from the repository root, without arguments, locally under `slot task check` and in CI.
- A check that builds or tests stays inside the pool job of `slot task check`; it ends within the
  gate's budget (`~/beyond10x/AGENTS.md` "Tests stay fast"). It makes no model call and no call to
  a real API.
- It may download or install what it needs; it writes its files under `target/` and is idempotent.
- Regeneration from sources is not a check: it goes in `tools/slow.d/`.
- POSIX sh or a Rust binary; no Python.
