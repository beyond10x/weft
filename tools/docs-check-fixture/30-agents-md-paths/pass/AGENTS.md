# Fixture of tools/test-docs-check.sh, not instructions

## Layout

- `ess/`: the model. `system.yaml`, `<layer>/<name>.yaml`, `scenarios/`.
- `tools/gen.sh`: writes `out/generated/` and reads `docs/a.md:3`.
- Not repository paths: the formats `ess-http/1` and `loom.agent/1`, the package `@scope/name`,
  `~/elsewhere/file`, `/usr/bin/env`, `https://example.org/a/b`, `$HOME/x`, `.../x/y`, `docs/*.md`
  and the template `<dir>/x.yaml`.
- A span that wraps the line: `docs/a.md
  --flag`.

```sh
cat missing/in/a/fence.txt
```

### Planned

- `surfaces/api.yaml`: not written yet.

#### Under the planned heading

- `surfaces/cli.yaml`: not written yet.

## After the planned part

- `docs/`: exists.
