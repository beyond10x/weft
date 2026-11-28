---
format: aep.planning-md/3
id: story:cloud-blocks-account-scope
kind: story
status: draft
title: A weft account opens its own cloudspec scope, and scope-keyed rows reference the scope
relations:
- decomposes: epic:cloud-blocks
- serves: vision:weft
- depends_on: story:cloud-blocks-vendor
revision: 1
---
## Epic

Decomposes `epic:cloud-blocks`: decision D-d, a weft account and a cloudspec scope.

## Outcome

1. `weft.accounts.Account` keeps `AccountId` and gains a unique `scope_id` reference to
   `weft.scope.Scope` (ESS refuses one identity type for both).
2. A binding `AccountCreated -> OpenScope` opens the account's scope.
3. Run-time records and example rows that the blocks key by scope reference Scope; account-only
   rows (roles, quotas, tokens) keep `account_id`.

## Scope

`ess/0-foundation/shared.yaml`, `ess/2-tenancy/accounts.yaml`, `ess/9-actors/actors.yaml`, and
the `references` lines of the entities that move to Scope (one line per file, coordinated through
`tools/check-overlay.sh`).

## Acceptance

1. `ess specify validate --path ess --strict-requires` reports no `conflicting_declaration` and no
   `type_mismatch` on Account or Scope.
2. The conformance scenario of `CreateAccount` shows `ScopeOpened` for the new account's
   `scope_id`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
