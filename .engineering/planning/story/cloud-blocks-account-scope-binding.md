---
format: aep.planning-md/3
id: story:cloud-blocks-account-scope-binding
kind: story
status: draft
title: An account's scope is recorded by a binding, and AccountTree shows it
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-rust
scope:
- confidence: cited
  path: ess/2-tenancy/accounts.yaml
- confidence: cited
  path: ess/components.yaml
- confidence: cited
  path: product/cloud/crates/weft-controllers/src/placement.rs
- confidence: cited
  path: product/cloud/crates/weft-impl/src/bindings
- confidence: cited
  path: product/cloud/crates/weft-testkit/src/steps.rs
revision: 6
---
## Epic

Decomposes `epic:cloud-blocks`. Found by `story:cloud-blocks-rust`.

## Found

- Binding `account-opens-scope` opens a scope when an account is created, but no control-plane
  step calls `weft.accounts.RecordAccountScope`: the account's `scope_id` is set only by the
  testkit step `record_account_scope` after `CreateAccount`.
- No view shows which scope an account holds. The placement controller reads `AccountStorage`
  directly to find the tenant of a scope (`weft-controllers/src/placement.rs`, commented as a
  stopgap). A patch adding `scope_id` to `weft.accounts.AccountTree` is at
  `$S/integ/r3/cloud-blocks-rust/0001-account-tree-scope-id.patch` in the coordinator's scratch.

## Outcome

1. A binding on `ScopeOpened` of an account's scope calls `RecordAccountScope`; the testkit step
   is removed.
2. `weft.accounts.AccountTree` rows carry `scope_id`; the placement controller reads the view.

## Scope

`ess/2-tenancy/accounts.yaml`, `ess/components.yaml`,
`product/cloud/crates/weft-controllers/src/placement.rs`,
`product/cloud/crates/weft-testkit/src/steps.rs`, `product/cloud/crates/weft-impl/src/bindings/`.

## Acceptance

1. A testkit test creates an account and reads its `scope_id` from `AccountTree` with no
   `record_account_scope` step.
2. `grep -n AccountStorage product/cloud/crates/weft-controllers/src/placement.rs` prints nothing.
3. `slot --here task test` exits 0 within 120 s.
