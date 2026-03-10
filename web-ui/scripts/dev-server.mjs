import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const srcDir = path.resolve(__dirname, '../src');
const port = Number(process.env.PORT ?? 5173);

const apiBaseUrl = process.env.HAWKBOT_UI_API_BASE_URL ?? 'http://localhost:8080';

const contentTypes = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8'
};

createServer(async (req, res) => {
  try {
    const requestPath = req.url === '/' ? '/index.html' : req.url;

    if (requestPath === '/config.js') {
      res.setHeader('Content-Type', contentTypes['.js']);
      res.end(`window.HAWKBOT_CONFIG = { apiBaseUrl: ${JSON.stringify(apiBaseUrl)} };`);
      return;
    }

    const filePath = path.resolve(srcDir, `.${requestPath}`);

    if (!filePath.startsWith(srcDir)) {
      res.statusCode = 403;
      res.end('Forbidden');
      return;
    }

    const ext = path.extname(filePath);
    const body = await readFile(filePath);
    res.setHeader('Content-Type', contentTypes[ext] ?? 'application/octet-stream');
    res.end(body);
  } catch {
    res.statusCode = 404;
    res.end('Not found');
  }
}).listen(port, () => {
  process.stdout.write(`hawkbot-web-ui dev server running on http://localhost:${port}\n`);
});
