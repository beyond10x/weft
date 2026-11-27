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
- Its time counts against the step limit (`WEFT_STEP_LIMIT`, at most 120 s) together with the
  tests: a setup that runs past the limit is ended and the step exits 124. A download that does
  not fit belongs in a CI or host preparation outside the step, not here.
- POSIX sh or a Rust binary; no Python.
- Passes a value to the tests through the environment the root `.cargo/config.toml` sets
  (`[env]`, a path with `relative = true`): a hook runs in a process of its own and cannot export
  a variable to the test processes. The hook reads the same variable, with that value as its
  default. `20-substrate.sh`: `WEFT_SUBSTRATE_SOCKET`, default `target/substrate/sock`.
