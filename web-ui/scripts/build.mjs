import { cp, mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');
const srcDir = path.join(rootDir, 'src');
const distDir = path.join(rootDir, 'dist');

const apiBaseUrl = process.env.HAWKBOT_UI_API_BASE_URL ?? 'http://localhost:8080';

await mkdir(distDir, { recursive: true });
await cp(srcDir, distDir, { recursive: true });
await writeFile(
  path.join(distDir, 'config.js'),
  `window.HAWKBOT_CONFIG = { apiBaseUrl: ${JSON.stringify(apiBaseUrl)} };\n`
);

process.stdout.write(`Built static assets into ${distDir}\n`);
