# weft

A compute cloud you deploy yourself. One control plane and any number of worker nodes run
confined workloads: a code snippet, a container image, a virtual machine, a model, an ESS system
or an agent. Workloads claim GPUs, disks, network addresses and secrets. Tenants, nested accounts
and quotas keep users apart.

Status: specification first. The design is [`docs/domain-sketch.md`](docs/domain-sketch.md); the
ESS specification is under [`ess/`](ess/). No code exists yet.

```console
task check
```

validates the specification.
