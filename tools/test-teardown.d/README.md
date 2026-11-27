# Test teardown hooks

`tools/test-step.sh` runs every executable file of this directory in name order
(`tools/run-hooks.sh`) after the tests: also after a failed run, a run the step limit ended, a
failed setup hook, or a SIGINT, SIGTERM or SIGHUP to the step (which ends the setup or the tests
first). The teardown hooks together get at most 30 s. A failing teardown hook fails the step when
the tests passed, and so does a step whose teardown ends past the step limit (`WEFT_STEP_LIMIT`).

Contract of a hook:

- Runs from the repository root, without arguments.
- Stops what a setup hook or a test started (a daemon, a socket) and collects its logs under
  `target/`.
- Is idempotent and succeeds when there is nothing to stop.
- POSIX sh or a Rust binary; no Python.
