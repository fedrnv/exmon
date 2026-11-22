// Local workspace acceptance check; never invoked from hosted workflows.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';

const allowed = {
  'exmon-domain': [],
  'exmon-capture': ['exmon-domain'],
  'exmon-protocol': ['exmon-domain'],
  'exmon-storage': ['exmon-domain'],
  'exmon-app': ['exmon-domain', 'exmon-capture', 'exmon-protocol', 'exmon-storage'],
  'exmon-gui': ['exmon-domain', 'exmon-app'],
};

function validate(packages, nodes, members) {
  const byId = new Map(packages.map(p => [p.id, p]));
  const graph = new Map(nodes.map(n => [n.id, n.dependencies]));
  assert.deepEqual(members.map(id => byId.get(id)?.name).sort(), Object.keys(allowed).sort());
  for (const id of members) {
    const name = byId.get(id).name;
    for (const dependency of graph.get(id) ?? []) {
      const target = byId.get(dependency)?.name;
      assert.ok(target, `Unresolved dependency ${dependency}`);
      if (target in allowed) {
        assert.ok(allowed[name].includes(target), `Forbidden edge ${name} -> ${target}`);
      }
    }
    if (name === 'exmon-gui') continue;
    const visited = new Set();
    const pending = [...(graph.get(id) ?? [])];
    while (pending.length) {
      const current = pending.pop();
      if (visited.has(current)) continue;
      visited.add(current);
      const target = byId.get(current)?.name;
      assert.ok(target, `Unresolved dependency ${current}`);
      assert.ok(!['exmon-gui', 'egui', 'eframe'].includes(target), `${name} reaches GUI ${target}`);
      pending.push(...(graph.get(current) ?? []));
    }
  }
}

// Prove rejection using isolated graph fixtures without modifying manifests.
const fixturePackages = Object.keys(allowed).map(name => ({ id: name, name }));
const fixtureNodes = Object.entries(allowed).map(([id, dependencies]) => ({ id, dependencies }));
const members = fixturePackages.map(p => p.id);
validate(fixturePackages, fixtureNodes, members);
for (const [source, target] of [['exmon-capture', 'exmon-gui'], ['exmon-protocol', 'exmon-storage']]) {
  assert.throws(() => validate(fixturePackages, fixtureNodes.map(n => n.id === source
    ? { ...n, dependencies: [...n.dependencies, target] } : n), members), /Forbidden edge/);
}
assert.throws(() => validate([...fixturePackages, { id: 'bridge', name: 'bridge' }, { id: 'egui', name: 'egui' }],
  [...fixtureNodes.map(n => n.id === 'exmon-domain' ? { ...n, dependencies: ['bridge'] } : n),
    { id: 'bridge', dependencies: ['egui'] }, { id: 'egui', dependencies: [] }], members), /reaches GUI/);

const result = spawnSync('cargo', ['metadata', '--format-version', '1', '--locked'], {
  encoding: 'utf8', timeout: 60000, maxBuffer: 16 * 1024 * 1024,
});
assert.equal(result.status, 0, result.error?.message ?? result.stderr);
const metadata = JSON.parse(result.stdout);
assert.ok(metadata.resolve, 'Cargo returned no resolved graph');
validate(metadata.packages, metadata.resolve.nodes, metadata.workspace_members);
console.log('PASS: six workspace layers, resolved dependency graph and three negative graph fixtures');
