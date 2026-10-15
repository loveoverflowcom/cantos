// Optional primitive for xtask's spike; never installs anything or captures a browser.
import fs from 'node:fs';
import {performance} from 'node:perf_hooks';
import looksSame from 'looks-same';
import pixelmatch from 'pixelmatch';
import {PNG} from 'pngjs';
import {compare as odiff} from 'odiff-bin';

const [manifest, output] = process.argv.slice(2);
const pairs = JSON.parse(fs.readFileSync(manifest, 'utf8'));
const records = [];
for (const pair of pairs) {
  for (const name of ['looks-same', 'pixelmatch', 'odiff']) {
    const times = [];
    let pixels = 0;
    for (let trial = 0; trial < 6; trial++) {
      const started = performance.now();
      const diffPath = `${output}/candidate-diff.png`;
      if (name === 'looks-same') {
        const result = await looksSame(pair.reference, pair.actual, {
          createDiffImage: true, shouldCluster: true, ignoreCaret: false,
        });
        pixels = result.equal ? 0 : result.differentPixels;
        if (result.diffImage) await result.diffImage.save(diffPath);
      } else if (name === 'pixelmatch') {
        const a = PNG.sync.read(fs.readFileSync(pair.reference));
        const b = PNG.sync.read(fs.readFileSync(pair.actual));
        const diff = new PNG({width: a.width, height: a.height});
        pixels = pixelmatch(a.data, b.data, diff.data, a.width, a.height, {
          threshold: .1, includeAA: false,
        });
        fs.writeFileSync(diffPath, PNG.sync.write(diff));
      } else {
        const result = await odiff(pair.reference, pair.actual, diffPath, {
          threshold: .1, antialiasing: true,
        });
        pixels = result.match ? 0 : result.diffCount;
      }
      if (!Number.isInteger(pixels)) throw new Error(`Invalid ${name} result`);
      if (trial) times.push(performance.now() - started);
    }
    times.sort((a, b) => a - b);
    records.push({id: pair.id, name, diff_pixels: pixels, median_ms: +times[2].toFixed(2)});
  }
}
fs.writeFileSync(`${output}/node-results.json`, JSON.stringify(records, null, 2) + '\n');
