import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const itemTypes = new Set(['task_closed', 'bug_closed', 'correction_closed']);
const requestTypes = new Set([...itemTypes, 'epic_closed', 'release_published', 'major_changed']);
const json = value => JSON.stringify(value, null, 2) + '\n';
const fail = message => { throw new Error(message); };
const nonempty = value => typeof value === 'string' && value.trim().length > 0;
const listJson = dir => fs.existsSync(dir) ? fs.readdirSync(dir).filter(name => name.endsWith('.json')).sort() : [];
const readJson = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const digest = value => crypto.createHash('sha256').update(value).digest('hex');

function frontMatter(file) {
  const source = fs.readFileSync(file, 'utf8');
  const header = source.match(/^\uFEFF?---\r?\n([\s\S]*?)\r?\n---/);
  if (!header) fail(`Missing front matter: ${file}`);
  const result = { source };
  for (const key of ['id', 'epic_id']) {
    const match = header[1].match(new RegExp(`^${key}:\\s*["']?([^"'\\r\\n]+)["']?\\s*$`, 'm'));
    if (match) result[key] = match[1].trim();
  }
  return result;
}

function walkRecords(root) {
  const epics = new Set();
  const items = new Map();
  const walk = dir => {
    if (!fs.existsSync(dir)) return;
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const file = path.join(dir, entry.name);
      if (entry.isSymbolicLink()) fail(`Symlink in task catalog: ${file}`);
      if (entry.isDirectory()) walk(file);
      else if (entry.name === 'TASK.md' || entry.name === 'EPIC.md') {
        const record = frontMatter(file);
        if (!nonempty(record.id) || epics.has(record.id) || items.has(record.id)) fail(`Invalid or duplicate identity: ${file}`);
        if (entry.name === 'EPIC.md') epics.add(record.id);
        else {
          let epicId = record.epic_id;
          if (!epicId) epicId = record.source.match(/^Epic:\s*([A-Za-z0-9._-]+)\s*$/m)?.[1];
          items.set(record.id, epicId ?? null);
        }
      }
    }
  };
  walk(path.join(root, 'Tasks', 'epics'));
  walk(path.join(root, 'Tasks', 'items'));
  return { epics, items };
}

function tuple(value, context) {
  const parts = String(value ?? '').split('.').map(Number);
  if (parts.length !== 4 || parts.some(part => !Number.isSafeInteger(part) || part < 0)) fail(`Invalid version tuple: ${context}`);
  return parts;
}

function transition(version, request) {
  const next = [...version];
  if (itemTypes.has(request.type)) next[3]++;
  else if (request.type === 'epic_closed') { next[2]++; next[3] = 0; }
  else if (request.type === 'release_published') { next[1]++; next[2] = 0; next[3] = 0; }
  else {
    if (!Number.isSafeInteger(request.major) || request.major <= next[0]) fail('major must increase');
    next.splice(0, 4, request.major, 0, 0, 0);
  }
  if (!next.every(Number.isSafeInteger)) fail('Version integer overflow');
  return next;
}

function validateRequest(request, file, catalog, historical = false) {
  if (request.schema !== 'fdtasks-version-request/v1') fail(`${file}: unsupported schema`);
  if (!nonempty(request.request_id) || !/^[A-Za-z0-9._-]+$/.test(request.request_id)) fail(`${file}: invalid request_id`);
  if (!requestTypes.has(request.type)) fail(`${file}: invalid type`);
  if (!nonempty(request.summary) || !Array.isArray(request.verification) || !request.verification.length || !request.verification.every(nonempty)) fail(`${file}: missing summary or verification`);
  if (request.type === 'major_changed') {
    if (!Number.isSafeInteger(request.major) || request.major < 0 || !nonempty(request.authorization)) fail(`${file}: major requires integer and authorization`);
    return;
  }
  if (historical) return;
  if (!catalog.epics.has(request.epic_id)) fail(`${file}: unknown Epic`);
  if (itemTypes.has(request.type)) {
    if (!catalog.items.has(request.work_item_id)) fail(`${file}: unknown work item`);
    const parent = catalog.items.get(request.work_item_id);
    if (parent && parent !== request.epic_id) fail(`${file}: work item belongs to another Epic`);
  }
}

function validateExisting(events, files) {
  if (!events.length) fail('Missing adoption version event');
  const seenRequests = new Set();
  const closedItems = new Set();
  const closedEpics = new Set();
  let version = tuple(events[0].version, files[0]);
  if (events[0].sequence !== 0) fail('Adoption event must have sequence 0');
  for (let index = 1; index < events.length; index++) {
    const event = events[index];
    if (event.sequence !== index) fail(`Version event sequence gap: ${files[index]}`);
    if (!requestTypes.has(event.type)) fail(`Invalid version event type: ${files[index]}`);
    if (!nonempty(event.request_id) || seenRequests.has(event.request_id)) fail(`Duplicate/missing event request_id: ${files[index]}`);
    seenRequests.add(event.request_id);
    if (itemTypes.has(event.type)) {
      if (!nonempty(event.work_item_id)) fail(`Missing work-item identity: ${files[index]}`);
      closedItems.add(event.work_item_id);
    }
    if (event.type === 'epic_closed') {
      if (!nonempty(event.epic_id)) fail(`Missing Epic identity: ${files[index]}`);
      closedEpics.add(event.epic_id);
    }
    version = transition(version, event);
    if (event.version !== version.join('.')) fail(`Version transition mismatch: ${files[index]}`);
  }
  return { version, seenRequests, closedItems, closedEpics };
}

function atomicWrite(file, content) {
  const temporary = `${file}.tmp-${process.pid}-${crypto.randomUUID()}`;
  fs.writeFileSync(temporary, content, { flag: 'wx' });
  fs.renameSync(temporary, file);
}

export function allocate(root = process.cwd()) {
  const requestDir = path.join(root, 'Tasks', 'version-requests');
  const eventDir = path.join(root, 'Tasks', 'version-events');
  const statePath = path.join(root, 'Tasks', 'version.json');
  const historyPath = path.join(root, 'Tasks', 'VERSION_HISTORY.md');
  const catalog = walkRecords(root);

  const eventFiles = listJson(eventDir);
  const events = eventFiles.map(file => readJson(path.join(eventDir, file)));
  const accepted = validateExisting(events, eventFiles);
  const state = readJson(statePath);
  if (state.schema !== 'fdtasks-version-state/v1' || state.version !== accepted.version.join('.') || state.sequence !== events.length - 1) fail('State does not match immutable version ledger');

  const requests = listJson(requestDir).map(file => {
    const raw = fs.readFileSync(path.join(requestDir, file), 'utf8');
    const request = JSON.parse(raw);
    validateRequest(request, file, catalog, accepted.seenRequests.has(request.request_id));
    return { file, request, hash: digest(raw) };
  });
  const byRequest = new Map();
  const byItem = new Map();
  const byEpic = new Map();
  for (const entry of requests) {
    const request = entry.request;
    if (byRequest.has(request.request_id)) fail(`Duplicate request_id: ${request.request_id}`);
    byRequest.set(request.request_id, entry);
    if (itemTypes.has(request.type)) {
      const ids = byItem.get(request.work_item_id) ?? new Set();
      ids.add(request.request_id);
      byItem.set(request.work_item_id, ids);
    }
    if (request.type === 'epic_closed') {
      const ids = byEpic.get(request.epic_id) ?? new Set();
      ids.add(request.request_id);
      byEpic.set(request.epic_id, ids);
    }
  }
  for (const requestId of accepted.seenRequests) if (!byRequest.has(requestId)) fail(`Accepted request is missing: ${requestId}`);
  for (const itemId of accepted.closedItems) if (!byItem.has(itemId)) fail(`Accepted work-item request is missing: ${itemId}`);
  for (const epicId of accepted.closedEpics) if (!byEpic.has(epicId)) fail(`Accepted Epic request is missing: ${epicId}`);
  for (const [itemId, ids] of byItem) {
    const pending = [...ids].filter(id => !accepted.seenRequests.has(id));
    if (pending.length > 1 || (pending.length && accepted.closedItems.has(itemId))) fail(`Duplicate permanent work_item_id: ${itemId}`);
  }
  for (const [epicId, ids] of byEpic) {
    const pending = [...ids].filter(id => !accepted.seenRequests.has(id));
    if (pending.length > 1 || (pending.length && accepted.closedEpics.has(epicId))) fail(`Duplicate permanent Epic closure: ${epicId}`);
  }

  let version = [...accepted.version];
  for (const entry of requests) {
    const request = entry.request;
    if (accepted.seenRequests.has(request.request_id)) continue;
    if (itemTypes.has(request.type) && accepted.closedItems.has(request.work_item_id)) fail(`Work item already closed: ${request.work_item_id}`);
    if (request.type === 'epic_closed' && accepted.closedEpics.has(request.epic_id)) fail(`Epic already closed: ${request.epic_id}`);
    version = transition(version, request);
    const event = {
      schema: 'fdtasks-version-event/v1',
      sequence: events.length,
      version: version.join('.'),
      request_id: request.request_id,
      type: request.type,
      epic_id: request.epic_id ?? '',
      work_item_id: request.work_item_id ?? '',
      summary: request.summary,
      verification: request.verification,
      request_file: `Tasks/version-requests/${entry.file}`,
      request_sha256: entry.hash,
    };
    const name = `${String(event.sequence).padStart(8, '0')}-${event.request_id}.json`;
    fs.writeFileSync(path.join(eventDir, name), json(event), { flag: 'wx' });
    events.push(event);
    accepted.seenRequests.add(request.request_id);
    if (itemTypes.has(request.type)) accepted.closedItems.add(request.work_item_id);
    if (request.type === 'epic_closed') accepted.closedEpics.add(request.epic_id);
  }

  const nextState = { schema: 'fdtasks-version-state/v1', version: version.join('.'), major: version[0], release: version[1], epic: version[2], item: version[3], sequence: events.length - 1 };
  const escape = value => String(value ?? '').replaceAll('|', '\\|').replace(/[\r\n]+/g, ' ');
  const history = '# FDTasks version history\n\nGenerated by the allocator. Do not edit manually.\n\n| Version | Type | Epic | Work item | Summary |\n|---|---|---|---|---|\n' + events.map(event => `| ${[event.version, event.type, event.epic_id, event.work_item_id, event.summary].map(escape).join(' | ')} |`).join('\n') + '\n';
  atomicWrite(statePath, json(nextState));
  atomicWrite(historyPath, history);
  return nextState;
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) console.log(JSON.stringify(allocate()));
