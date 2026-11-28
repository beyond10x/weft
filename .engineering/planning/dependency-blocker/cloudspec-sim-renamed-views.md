---
format: aep.planning-md/3
id: dependency-blocker:cloudspec-sim-renamed-views
kind: dependency-blocker
status: cleared
title: cloudspec-sim at a485b7f reads view names from before the rename, so the closed-link experiment exits 2
relations:
- blocks: story:cloud-blocks-sim-pin
revision: 2
transitions:
- {from: "open", to: "cleared", at: "2026-11-28T11:11:55Z", actor: "human:operator", revision: 2}
---
# cloudspec-sim at a485b7f reads view names from before the rename, so the closed-link experiment exits 2
