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
scope:
- confidence: cited
  path: ess/0-foundation/shared.yaml
- confidence: cited
  path: ess/2-tenancy/accounts.yaml
- confidence: cited
  path: ess/9-actors/actors.yaml
- confidence: cited
  path: ess/components.yaml
revision: 3
---
## Epic

Decomposes `epic:cloud-blocks`: decision D-d, a weft account and a cloudspec scope.

## Outcome

1. `weft.accounts.Account` keeps `AccountId` and gains a unique `scope_id` reference to
   `weft.scope.Scope` (ESS refuses one identity type for both).
2. A binding `AccountCreated -> OpenScope` opens the account's scope.
3. `OpenScope` and `CloseScope` are granted to weft's actors.
4. Run-time records and example rows that the blocks key by scope reference Scope; account-only
   rows (roles, quotas, tokens) keep `account_id`. The 8 commands that clash today
   (RevokeAccountApiTokens, RevokeAccountExports, StopAccountInstances and 5 vendored scope
   commands) validate.

## Scope

`ess/0-foundation/shared.yaml`, `ess/2-tenancy/accounts.yaml`, `ess/9-actors/actors.yaml`,
`ess/components.yaml` (the new binding only), and the `references` lines of the entities that move
to Scope.

## Acceptance

1. `ess specify validate --path ess --strict-requires` reports no `conflicting_declaration` and no
   `type_mismatch` on Account or Scope.
2. The compiled model (`ess specify compile --path ess --format json`) has a binding from
   `weft.accounts.AccountCreated` to `weft.scope.OpenScope`, and grants of `OpenScope` and
   `CloseScope` to weft actors.
3. `slot --timeout 30m tools/synth-domain.sh accounts` (about 80 s; outside the test step) reports
   0 failed scenarios of `weft.accounts`.

## Rules for this work

- Merge into the integration branch `move/vendor-cloud-blocks`; `main` gets it once the whole gate
  is green.
- Builds and tests run through `slot --here`. No test over 60 s; the test step ends within 120 s. A
  pool job ends within 10 minutes unless it names its time with `slot --timeout` (at most 30m). No
  Python in anything committed. No network request in the test step.
- One owner per domain file; parallel edits are checked with `tools/check-overlay.sh`.
