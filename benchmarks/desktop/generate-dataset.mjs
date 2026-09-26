// Reproduce QuickGUI's published dataset byte-for-byte (no TypeScript runtime required).
import { readFileSync, writeFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { deflateRawSync, inflateRawSync } from 'node:zlib';
import vm from 'node:vm';
const root = new URL('./', import.meta.url);
const source = readFileSync(new URL('reference/workload.ts', root), 'utf8')
  .replaceAll('export ', '').replace(' as const', '').replaceAll(']!', ']');
const dataset = vm.runInNewContext(`${source}\nJSON.stringify(createIssues(), null, 2)`);
const hash = createHash('sha256').update(dataset).digest('hex');
const reference = JSON.parse(readFileSync(new URL('reference/quickgui-macos.json', root)));
if (hash !== reference.workload.datasetSha256) throw new Error(`Dataset mismatch: ${hash}`);
writeFileSync(new URL('../../examples/issue_tracker/issues.json', root), dataset);
const packed = deflateRawSync(Buffer.from(dataset), { level: 9 });
if (inflateRawSync(packed).toString('utf8') !== dataset) throw new Error('Dataset compression mismatch');
writeFileSync(new URL('../../examples/issue_tracker/issues.json.deflate', root), packed);
console.log(`Verified 1,000 issues; SHA-256 ${hash}`);
