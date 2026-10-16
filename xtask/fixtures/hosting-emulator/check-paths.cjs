// Local contract probe against an operator-installed CLI, never a provider invocation.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const moduleRoot = process.argv[2];
assert(moduleRoot && path.isAbsolute(moduleRoot), 'pass the absolute firebase-tools package path');
assert.equal(require(path.join(moduleRoot, 'package.json')).version, '15.34.0');
const { Config } = require(path.join(moduleRoot, 'lib/config.js'));
const { detectProjectRoot } = require(path.join(moduleRoot, 'lib/detectProjectRoot.js'));
const { resolveProjectPath } = require(path.join(moduleRoot, 'lib/projectPath.js'));
const root = path.resolve(__dirname, '../../..');
const configPath = path.join(root, 'infra/firebase/hosting/firebase.json');
const policy = JSON.parse(fs.readFileSync(configPath, 'utf8'));
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'cantos Firebase paths '));

try {
  const stage = path.join(scratch, 'run with spaces');
  fs.mkdirSync(stage);
  const effective = path.join(stage, 'firebase.json');
  fs.writeFileSync(effective, JSON.stringify({ hosting: [{ ...policy.hosting[0], public: 'dist' }] }));
  for (const cwd of [root, path.dirname(configPath), scratch]) {
    const options = { cwd, configPath };
    assert.equal(detectProjectRoot(options), path.dirname(configPath));
    const config = new Config(policy, options);
    for (const hosting of policy.hosting) {
      assert.equal(
        resolveProjectPath(options, hosting.public),
        path.join(root, 'apps', `cantos-${hosting.target}`, 'web/dist'),
      );
      assert.throws(() => config.path(hosting.public), /outside of project directory/);
    }
    const staged = new Config(JSON.parse(fs.readFileSync(effective, 'utf8')), {
      cwd, configPath: effective,
    });
    assert.equal(staged.path('dist'), path.join(stage, 'dist'));
    console.log(`PASS explicit config from ${cwd}: diagnostic app paths, traversal refusal, staged dist`);
  }
  console.log('PASS firebase-tools 15.34.0 actual configuration modules; no provider invocation');
} finally {
  fs.rmSync(scratch, { recursive: true });
}
