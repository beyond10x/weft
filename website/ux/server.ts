// Serves the built site (`website/build`) under its base URL on 127.0.0.1, as a static host does: the
// exact file, else `<path>.html`, else `<path>/index.html`, else `404.html` with status 404. The
// harness's negative fixtures are served under `/__ux/fixtures/`, outside the site. Nothing but the
// build directory and the fixtures is read.
//
//   node ux/server.ts <port> <build dir>

import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { dirname, extname, join, normalize, resolve } from 'node:path';

const port = Number(process.argv[2] ?? 4327);
const root = resolve(process.argv[3] ?? join(dirname(resolve(process.argv[1]!)), '..', 'build'));
const fixtures = join(dirname(resolve(process.argv[1]!)), 'fixtures');
const base = '/weft/';

const TYPES: Record<string, string> = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.woff2': 'font/woff2',
  '.woff': 'font/woff',
  '.xml': 'application/xml',
  '.txt': 'text/plain; charset=utf-8',
  '.ico': 'image/x-icon',
};

const file = (path: string): string | null => (existsSync(path) && statSync(path).isFile() ? path : null);

function under(dir: string, rest: string): string | null {
  const clean = normalize(decodeURIComponent(rest)).replace(/^(\.\.(\/|$))+/, '');
  const at = join(dir, clean);
  if (!at.startsWith(dir)) return null;
  if (clean === '' || clean === '.' || clean.endsWith('/')) return file(join(at, 'index.html'));
  return file(at) ?? file(`${at}.html`) ?? file(join(at, 'index.html'));
}

if (!existsSync(join(root, 'index.html'))) {
  console.error(`server: no built site at ${root}; build it first (npm --prefix website run build)`);
  process.exit(1);
}

createServer((req, res) => {
  const url = new URL(req.url ?? '/', `http://127.0.0.1:${port}`);
  if (url.pathname === '/__ux/health') {
    res.writeHead(200, { 'content-type': 'text/plain' }).end('ok');
    return;
  }
  const found = url.pathname.startsWith('/__ux/fixtures/')
    ? under(fixtures, url.pathname.slice('/__ux/fixtures/'.length))
    : url.pathname.startsWith(base)
      ? under(root, url.pathname.slice(base.length))
      : null;
  const path = found ?? join(root, '404.html');
  res.writeHead(found ? 200 : 404, { 'content-type': TYPES[extname(path)] ?? 'application/octet-stream', 'cache-control': 'no-store' });
  createReadStream(path).pipe(res);
}).listen(port, '127.0.0.1', () => console.log(`server: ${root} at http://127.0.0.1:${port}${base}`));

// The run that started this server may be killed outright (`timeout 120` sends it SIGTERM): the
// server then outlives it and holds the port. It exits once its parent is gone (it is re-parented).
const parent = process.ppid;
setInterval(() => {
  if (process.ppid !== parent) process.exit(0);
}, 500).unref();

