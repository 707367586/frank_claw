# picoclaw upstream provenance

This `backend/` tree is a flat copy of [sipeed/picoclaw](https://github.com/sipeed/picoclaw) source at:

| Field | Value |
|---|---|
| Source | https://github.com/sipeed/picoclaw |
| Branch | main |
| Commit SHA | `8461c996e5ad2f20801622a8eeec931f8966a066` |
| Commit date | 2026-04-20T03:18:42Z |
| Commit subject | chore(web): update linting and router dependencies (#2592) |
| License | MIT (see `backend/LICENSE`) |
| Imported on | 2026-04-20 |
| Importer | ClawX migration v0.4.0 (ADR-037 v2) |

## Excluded from import

- `.git`, `.github` — separate version control / CI flows
- `.gitignore` — replaced by repo root `.gitignore`
- `docker/` — ADR-037 v2 doesn't deploy via docker
- `.goreleaser*` — release pipeline is owned by this repo, not upstream

## Manual upstream sync (when needed)

1. Fetch new SHA: `git -C /tmp/picoclaw fetch && git -C /tmp/picoclaw checkout <new-sha>`
2. Three-way diff: `diff -ru /tmp/picoclaw/ backend/ > /tmp/upstream.diff`
3. Apply selectively, **always preserving the local changes recorded in `PATCHES.md`**.
4. Run full test suite (`go test ./backend/...` + `pnpm --filter clawx-gui vitest run`).
5. If green, update `Commit SHA` and `Commit date` above.

We do **not** use `git subtree` / `submodule`. Maintaining a clean linear history matters more than automatic upstream pulls.
