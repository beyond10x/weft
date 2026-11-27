# Test setup hooks

`tools/test-step.sh` runs every executable file of this directory in name order
(`tools/run-hooks.sh`) after the `rustc -V` check and before the tests; the first that fails ends
the step, and the teardown hooks still run.

Contract of a hook:

- Runs from the repository root, without arguments.
- Prepares what the tests need. It may download or install (it is no test, so the no-network rule
  of the test step does not bind it); a CI-only runner preparation, such as a sysctl, is written in
  the hook that needs it.
- Writes its files under `target/`.
- Is idempotent: a second run leaves the same state.
- Its time does not count against the step limit (`WEFT_STEP_LIMIT`); keep it short.
- POSIX sh or a Rust binary; no Python.
