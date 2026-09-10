# AGENTS.md

These rules apply to the entire repository and to human and AI contributors.

Before work:

1. read `README.md` and `DEVELOPMENT.md`;
2. identify the governing task under `Tasks/items/`;
3. read its immutable `TASK.md`, all task events, evidence, and linked ADRs;
4. follow `Tasks/AGENTS.md` for task records.

Development rules:

- Implement one accepted Work Item at a time.
- Work directly on `main`; do not create branches or pull requests unless the project owner explicitly requests them.
- Keep capture, framing, decoding, storage, and GUI layers separated.
- Keep transport-specific privileges and unsafe platform APIs behind small reviewed adapters.
- Never block capture or decoding on GUI rendering.
- Bound queues and memory use; make packet loss and dropped samples observable.
- Validate untrusted capture files, protocol definitions, and device data.
- Never transmit on a physical interface without an explicit user action or enabled schedule.
- Run relevant tests after every task, verify that existing behavior remains intact, remove generated build debris, commit, push to `main`, and proceed to the next unblocked task.
- Continue autonomous development until a physical device, OS permission, signing identity, credential, or product-owner decision is required.
- GitHub Actions must be manual-only until the project owner changes this policy.
- Code comments and technical documentation are written in English.
- Never commit credentials, packet captures containing sensitive data, device identifiers, signing material, or production secrets.
