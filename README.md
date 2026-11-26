<p align="center">
  <img src="docs/assets/weft-logo.svg" alt="weft: the compute cloud you weave yourself" width="560">
</p>

<p align="center">
  A compute cloud you deploy yourself, specified first in ESS.
</p>

---

One control plane and any number of worker nodes run confined workloads: a code snippet, a
container image, a virtual machine, a model, an ESS system or an agent. Workloads claim GPUs,
disks, network addresses and secrets. Tenants, nested accounts, tenant clusters and quotas keep
users apart.

Status: specification first. No code exists yet.

| Where | What |
|---|---|
| [`docs/domain-sketch.md`](docs/domain-sketch.md) | The design every specification file follows |
| [`ess/`](ess/) | The ESS specification: 23 domains in 10 layers, components and bindings |
| [`tools/sim/`](tools/sim/) | weft-sim: explores and simulates the specification in memory |
| [`sim/experiments/`](sim/experiments/) | Experiments: a full setup, faults, duplicate delivery, topology |
| [`docs/hardening/`](docs/hardening/) | The hardening passes and what each changed |
| [`quality/ratchet.yaml`](quality/ratchet.yaml) | The KPIs a change may not make worse |

```console
slot task check
```

validates the specification, its layers and stamped time, and runs weft-sim's tests.

```console
task sim-explore-from
```

explores every command from the full setup and checks the system invariants.
