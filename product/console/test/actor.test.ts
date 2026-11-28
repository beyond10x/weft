// What the signed-in actor decides in the renderer: the model actors its roles name (by wire name),
// the pages and actions their grants allow (`navigation.visibility: by_actor_grants`, `may`), the
// inputs it owns, and the params a `label_from` read takes from it.

import { describe, expect, it } from 'vitest';
import { compileModel, loadBundle } from '../build/bundle.ts';
import { actionAllowed, callerOwned, grantsOf, pageAllowed } from '../src/renderer/grants.ts';
import { labelFromParams } from '../src/renderer/runtime.tsx';
import type { Action } from '../src/renderer/types.ts';

const ir = compileModel();
const account = loadBundle('account', [], ir);
const operator = loadBundle('operator', [], ir);

describe('grants', () => {
  const full = grantsOf(account.model, account.actor);
  const deployer = grantsOf(account.model, account.actors.deployer);

  it('the roles of an actor name model actors by their wire name', () => {
    expect([...full.actors].sort()).toEqual(['weft.actors.AccountAdmin', 'weft.actors.Deployer']);
    expect([...deployer.actors]).toEqual(['weft.actors.Deployer']);
    expect([...grantsOf(operator.model, operator.actor).actors]).toEqual(['weft.actors.ClusterOperator']);
    expect(full.commands.has('weft.formations.CreateFormation')).toBe(true);
    expect(deployer.commands.has('weft.formations.CreateFormation')).toBe(false);
    expect(deployer.commands.has('weft.workloads.ScaleService')).toBe(true);
  });

  it('a page whose actor the signed-in actor does not hold is hidden', () => {
    expect(pageAllowed(account.doc, 'formations', full)).toBe(true);
    expect(pageAllowed(account.doc, 'formations', deployer)).toBe(false);
    expect(pageAllowed(account.doc, 'usage', deployer)).toBe(false);
    expect(pageAllowed(account.doc, 'home', deployer)).toBe(true);
    expect(pageAllowed(account.doc, 'formations.detail', deployer)).toBe(true);
    const op = grantsOf(operator.model, operator.actor);
    expect(Object.keys(operator.doc.pages).filter((p) => !pageAllowed(operator.doc, p, op))).toEqual([]);
  });

  it('an action whose command is not granted, or that opens such a form or a hidden page, is hidden', () => {
    const page = account.doc.pages.formations;
    const create = (page.header?.actions as Action[]).find((a) => a.name === 'create')!;
    const list = page.sections.find((s) => s.name === 'list')!;
    const retire = (list.row_actions as Action[]).find((a) => a.name === 'retire')!;
    expect(actionAllowed(account.doc, page, create, full)).toBe(true);
    expect(actionAllowed(account.doc, page, create, deployer)).toBe(false);
    expect(actionAllowed(account.doc, page, retire, deployer)).toBe(false);
    const toUsage: Action = { name: 'go', navigate: { to: 'usage' } };
    expect(actionAllowed(account.doc, page, toUsage, deployer)).toBe(false);
    expect(actionAllowed(account.doc, page, toUsage, full)).toBe(true);
  });

  it('the inputs a caller owns are account_id, tenant_id and the attributes of the actors it holds', () => {
    expect([...callerOwned(account.model, account.actor)].sort()).toEqual(['account_id', 'principal_id', 'tenant_id']);
  });
});

describe('label_from params', () => {
  it('a param of the view is bound from the actor by name, then from the page params', () => {
    expect(labelFromParams(account.model, 'weft.observability.AlertRules', account.actor, {})).toEqual({ account_id: account.actor.account_id });
    expect(labelFromParams(account.model, 'weft.secrets.SecretBackends', account.actor, {})).toEqual({});
    expect(labelFromParams(account.model, 'weft.formations.Environments', account.actor, { formation_id: 'f1' })).toEqual({ formation_id: 'f1' });
  });
  it('a param neither carries leaves the label map unread', () => {
    expect(labelFromParams(account.model, 'weft.formations.Environments', account.actor, {})).toBeUndefined();
  });
});
