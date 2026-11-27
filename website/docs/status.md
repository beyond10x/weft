---
title: Status
description: What exists in weft today and what is planned.
mdx:
  format: mdx
---

import {StatusTable} from '@beyond10x/docs-system/product';
import status from '@site/data/status.json';

**Shipped** exists in the repository and is held by the gate: every shipped item names a file or a
command in `website/data/status-proofs.json` that proves it, and the docs check runs them on every
pull request. **Planned** is not built; nothing planned runs today.

<StatusTable data={status} />

The table and its date are read from `website/data/status.json`, the same file the landing page
reads.
