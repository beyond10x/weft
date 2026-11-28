// Vite plugin: `import bundle from 'virtual:ess-ui/<app>'` yields the console's bundle
// (build/bundle.ts) as JSON, parsed from ui/<app>.ui.yaml at build time. In `vite dev` a change of
// the document or a fixture reloads the page.

import type { Plugin } from 'vite';
import { loadBundle } from './bundle.ts';

const PREFIX = 'virtual:ess-ui/';

export function essUi(): Plugin {
  return {
    name: 'ess-ui',
    resolveId(id) {
      return id.startsWith(PREFIX) ? `\0${id}` : undefined;
    },
    load(id) {
      if (!id.startsWith(`\0${PREFIX}`)) return undefined;
      const app = id.slice(PREFIX.length + 1);
      const files: string[] = [];
      const bundle = loadBundle(app, files);
      for (const f of files) this.addWatchFile(f);
      return `export default ${JSON.stringify(bundle)};`;
    },
  };
}
