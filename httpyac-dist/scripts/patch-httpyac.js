const fs = require('node:fs');
const path = require('node:path');

const packageDir = (name) => path.dirname(require.resolve(`${name}/package.json`));

const patches = [
  {
    // A refused `localhost` (tried over IPv6 and IPv4) fails with an AggregateError whose message
    // is empty; got copies it, so httpyac prints no reason. Use the inner errors' messages instead.
    name: 'aggregate-error-message',
    file: path.join(packageDir('got'), 'dist', 'source', 'core', 'index.js'),
    pattern: /(class RequestError extends Error \{\s*constructor\(message, error, self\) \{\s*var _a, _b;\s*)super\(message\);/g,
    guard: "this.name = 'RequestError';",
    replace: (_, head, marker) =>
      `${head}${marker}super(message || (Array.isArray(error && error.errors) ? error.errors.map((e) => e.message).join(', ') : message));`,
  },
];

for (const patch of patches) {
  const marker = `/*http-runner:${patch.name}*/`;
  let source = fs.readFileSync(patch.file, 'utf8');
  if (source.includes(marker)) {
    console.log(`already patched: ${patch.name}`);
    continue;
  }
  const matches = [...source.matchAll(patch.pattern)];
  if (matches.length !== 1 || !source.includes(patch.guard)) {
    console.error(`patch-httpyac: ${patch.name}: expected 1 match in ${patch.file}, found ${matches.length}; update the pattern for this version`);
    process.exit(1);
  }
  source = source.replace(patch.pattern, (...groups) => patch.replace(...groups.slice(0, -2), marker));
  fs.writeFileSync(patch.file, source);
  console.log(`patched: ${patch.name}`);
}
