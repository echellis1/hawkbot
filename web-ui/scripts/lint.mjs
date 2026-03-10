import { readFile } from 'node:fs/promises';

const files = ['src/index.html', 'src/styles.css', 'src/app.js'];

for (const file of files) {
  const content = await readFile(new URL(`../${file}`, import.meta.url), 'utf8');

  if (content.includes('\t')) {
    throw new Error(`${file} contains tab characters; use spaces.`);
  }
}

process.stdout.write('Lint checks passed.\n');
