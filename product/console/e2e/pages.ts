// Every page of both documents with the URL that opens it on fixture data: the page list comes from
// the documents themselves, the ids of detail pages from the fixtures.

import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { parse } from 'yaml';

const UI = fileURLToPath(new URL('../ui/', import.meta.url));

/** The fixture id each page param takes. */
const IDS: Record<string, string> = {
  node_id: '531fdf41-b814-464f-a3d8-0b4f9ed7bce8',
  tenant_id: 'a38fd8ba-fa25-42c4-9a58-974e92dcf880',
  formation_id: '5fb49110-c71c-4123-a85a-9edd0134d148',
  environment_id: 'f990dc15-e94f-446d-b08d-eeece15833ab',
  workload_id: '664cb8ca-6299-486d-9342-87a8a94c514c',
  instance_id: 'dbb0fcea-c300-4eae-98b3-a65c4ed77007',
  release_id: '5acd4b45-4054-48ac-a6c8-311403a1a10b',
  service_instance_id: '24d5994f-bdc3-4f08-ae40-2b08b8cd2576',
};

export interface PageCase {
  app: 'operator' | 'account';
  page: string;
  title: string;
  url: string;
}

export function pages(app: 'operator' | 'account'): PageCase[] {
  const doc = parse(readFileSync(`${UI}${app}.ui.yaml`, 'utf8'), { maxAliasCount: -1 });
  return Object.entries<Record<string, unknown>>(doc.pages).map(([name, p]) => {
    const params = Object.keys((p.params as Record<string, unknown>) ?? {});
    const segs = [...name.split('.'), ...params.map((k) => {
      if (!IDS[k]) throw new Error(`no fixture id for param ${k} of ${app} page ${name}`);
      return IDS[k];
    })];
    return { app, page: name, title: String(p.title ?? name), url: `/${app}/?latency=0#/${segs.join('/')}` };
  });
}

export const ALL: PageCase[] = [...pages('operator'), ...pages('account')];
