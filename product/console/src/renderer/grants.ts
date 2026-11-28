// What the signed-in actor may see and do (`navigation.visibility: by_actor_grants`, `actor.may`).
// The actor's `roles` are wire names of model actors; the actor holds those actors and their
// grants (`may`). The model grants commands only (views are not granted yet, an OPEN of
// ess/9-actors/actors.yaml), so a page shows when the actor holds the model actor the page names
// (`Page.actor`, the actor whose grants bind the commands it sends), and an action shows when its
// command is granted, the form it opens sends a granted command, and the page it opens shows.

import type { Action, Doc, Json, ModelMeta, Obj, Page } from './types.ts';

export interface Grants {
  /** The model actors the signed-in actor holds. */
  actors: Set<string>;
  /** Every command their grants name. */
  commands: Set<string>;
}

export function grantsOf(model: ModelMeta, actor: Obj | undefined): Grants {
  const roles = new Set<string>((Array.isArray(actor?.roles) ? actor!.roles : []).map(String));
  const actors = new Set<string>();
  const commands = new Set<string>();
  for (const [name, a] of Object.entries(model.actors ?? {})) {
    if (!roles.has(a.wire)) continue;
    actors.add(name);
    for (const c of a.may) commands.add(c);
  }
  return { actors, commands };
}

/** A page shows when it names no actor, or one the signed-in actor holds. */
export function pageAllowed(doc: Doc, name: string, g: Grants): boolean {
  const page = doc.pages[name];
  if (!page) return false;
  return !page.actor || g.actors.has(page.actor);
}

/** An action shows when what it does, opens or navigates to is allowed. `page` holds the overlays it opens. */
export function actionAllowed(doc: Doc, page: Page | undefined, a: Action, g: Grants): boolean {
  if (a.does && !g.commands.has(a.does)) return false;
  if (a.opens) {
    const o: Obj | undefined = page?.overlays?.[a.opens];
    if (o?.does && !g.commands.has(o.does)) return false;
  }
  if (a.navigate?.to && !pageAllowed(doc, a.navigate.to, g)) return false;
  if (a.upload?.does && !g.commands.has(a.upload.does)) return false;
  return true;
}

/** The inputs a caller owns: account_id, scope_id, tenant_id, and every attribute of the model actors it holds. */
export function callerOwned(model: ModelMeta, actor: Json): Set<string> {
  const out = new Set(['account_id', 'scope_id', 'tenant_id']);
  const held = grantsOf(model, actor).actors;
  for (const name of held) for (const at of model.actors[name]?.attributes ?? []) out.add(at);
  return out;
}
