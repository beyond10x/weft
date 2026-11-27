# Test teardown hooks

`tools/test-step.sh` runs every executable file of this directory in name order
(`tools/run-hooks.sh`) after the tests: also after a failed run, a run the step limit ended, or a
failed setup hook. A failing teardown hook fails the step when the tests passed.

Contract of a hook:

- Runs from the repository root, without arguments.
- Stops what a setup hook or a test started (a daemon, a socket) and collects its logs under
  `target/`.
- Is idempotent and succeeds when there is nothing to stop.
- POSIX sh or a Rust binary; no Python.
