<p align="center">
  <img src="docs/assets/weft-logo.svg" alt="weft: the compute cloud you weave yourself" width="560">
</p>

<p align="center">
  A compute cloud you deploy yourself, specified first in ESS.
</p>

<p align="center">
  <a href="https://beyond10x.github.io/weft/"><strong>Documentation</strong></a>
</p>

---

One control plane and any number of worker nodes run confined workloads: a code snippet, a
container image, a virtual machine, a model, an ESS system or an agent. Workloads claim GPUs,
disks, network addresses and secrets. Tenants, nested accounts, tenant clusters and quotas keep
users apart.

weft is the runtime of [cloudspec](https://github.com/beyond10x/cloudspec) specifications: it runs
what cloudspec translated from a source (a Helm chart, a Compose file, an agenticspec deployment,
an ESS system) and has no translator of its own. The multi-tenant cloud described here is its first
example.

Status: specification first. No code exists yet.

| Where | What |
|---|---|
| [`docs/domain-sketch.md`](docs/domain-sketch.md) | The design every specification file follows |
| [`ess/`](ess/) | The ESS specification: 23 domains in 10 layers, components and bindings |
| [`product/cloud/crates/`](product/cloud/crates/) | weft's Rust crates; the code ESS generates from `ess/` is synthesized into `target/generated/`, not committed |
| [cloudspec-sim](https://github.com/beyond10x/cloudspec/tree/2ed72c2866bbd165da2de75250b2bfcb340a96ae/crates/cloudspec-sim) | Explores and simulates the specification in memory; a crate of cloudspec (weft-sim until 2026-11-28), installed by `task sim-install` at the revision `CLOUDSPEC_REV` of `Taskfile.yml` |
| [`sim/experiments/`](sim/experiments/) | Experiments: a full setup, faults, duplicate delivery, topology |
| [`docs/hardening/`](docs/hardening/) | The hardening passes and what each changed |
| [`quality/ratchet.yaml`](quality/ratchet.yaml) | The KPIs a change may not make worse |
| [`website/`](website/) | The documentation site, published at [beyond10x.github.io/weft](https://beyond10x.github.io/weft/) |

```console
slot task check
```

validates the specification, its layers and stamped time, and runs the tests of the root workspace.

```console
task sim-explore-from
```

installs cloudspec-sim at the pinned revision, explores every command from the full setup and checks
the system invariants.
