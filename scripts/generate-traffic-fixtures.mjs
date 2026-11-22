// Entire corpus is synthetic: no captures, randomness, clock, devices or network.
import fs from 'node:fs';
import assert from 'node:assert/strict';

const destination = new URL('../fixtures/traffic/corpus.json', import.meta.url);
const transports = {
  ethernet: { link_type: 1, layer: 'application_payload' },
  tcp: { connection_epoch: 1, layer: 'ordered_stream' },
  udp: { layer: 'datagram_payload' },
  serial: { baud: 9600, data_bits: 8, parity: 'none', stop_bits: 1 },
  usb: { endpoint: 129, transfer: 'interrupt', status: 'completed' },
  can: { arbitration_id: 291, extended: false, fd: false },
};
const modes = ['datagram', 'fixed', 'delimiter', 'length_prefix'];
const scenarios = ['normal', 'fragmented', 'malformed', 'truncated', 'burst', 'timestamped'];
const cases = [];
const encode = (mode, message) => mode === 'delimiter' ? [...message, 10]
  : mode === 'length_prefix' ? [message.length, ...message] : [...message];
for (const [transport, metadata] of Object.entries(transports)) {
  for (const framing of modes) {
    if (framing === 'datagram' && ['tcp', 'serial'].includes(transport)) continue;
    for (const scenario of scenarios) {
      const payload = [165, 0, 42];
      const frame = encode(framing, scenario === 'malformed' ? [164, 0, 42] : payload);
      let chunks = [frame];
      if (scenario === 'fragmented') chunks = [frame.slice(0, 1), frame.slice(1)];
      // A burst preserves adapter observations even for framed transports.
      if (scenario === 'burst') chunks = Array.from({ length: 8 }, () => [...frame]);
      if (scenario === 'truncated') chunks = [frame.slice(0, -1)];
      const observations = chunks.map((bytes, index) => ({
        sequence: index, timestamp_ns: scenario === 'timestamped' ? 123456789 : index * 1000,
        direction: 'Rx', bytes, original_length: scenario === 'truncated' ? frame.length : bytes.length,
      }));
      const successful = !['malformed', 'truncated'].includes(scenario)
        && !(scenario === 'fragmented' && framing === 'datagram');
      const expected = successful ? Array.from({ length: chunks.length === 8 ? 8 : 1 }, () => ({
        type_id: 'synthetic_value', bytes: payload, fields: { value: 42 },
      })) : [];
      cases.push({ id: `${transport}-${framing}-${scenario}`, transport, metadata,
        framing, scenario, observations, expected,
        outcome: scenario === 'truncated' ? 'capture_truncated'
          : scenario === 'malformed' ? 'unclassified'
            : !successful ? 'incomplete_datagrams' : 'complete' });
    }
  }
}
const header = { schema_version: 1, provenance: 'generated_synthetic_constants', license: 'CC0-1.0',
  protocol: { id: 'synthetic_value', prefix: [165], length: 3,
    field: { id: 'value', offset: 1, length: 2, endian: 'big' } },
  framing_rules: { fixed_length: 3, delimiter: [10], length_prefix_width: 1, length_excludes_prefix: true } };
// One case per line keeps the generated fixture reviewable and compact.
const output = `${JSON.stringify(header).slice(0, -1)},"cases":[\n${cases.map(c => JSON.stringify(c)).join(',\n')}\n]}\n`;
const args = process.argv.slice(2);
assert.ok(args.length === 1 && ['--check', '--write'].includes(args[0]), 'Use --check or --write');
if (args[0] === '--write') {
  fs.mkdirSync(new URL('../fixtures/traffic/', import.meta.url), { recursive: true });
  fs.writeFileSync(destination, output);
} else {
  assert.equal(fs.readFileSync(destination, 'utf8').replaceAll('\r\n', '\n'), output, 'Corpus drift; review generator and regenerate explicitly');
}
console.log(`PASS: ${cases.length} synthetic cases (${args[0]})`);
