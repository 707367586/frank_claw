# TypeScript / React Coding Rules

Scope: `apps/clawx-gui/` (React 19 + Vite 6 + TypeScript 5 + vitest 4).

## Tooling

- `pnpm` only. Never `npm` or `yarn`.
- `vitest run` for tests. `pnpm --filter clawx-gui test`.
- ESLint for lint (`pnpm --filter clawx-gui lint`). Use the config already in the repo; don't add plugins unilaterally.
- No `tsc --build` step — Vite handles the dev path; `vitest` type-checks during tests.

## Typing

- `strict: true` in tsconfig (already set). Don't weaken it.
- Explicit types on public function signatures, props, and exported values. Infer locals.
- No `any`. If you need to escape the type system, use `unknown` + narrowing, or a `// eslint-disable-next-line` with a one-line reason.
- Prefer `interface` for object shapes across module boundaries, `type` for unions/utility types.
- Discriminated unions for message types (`type: "message.create"`) — don't invent a second parallel system.

## React

- Functional components + hooks only. No class components.
- Keep components thin. Stateful logic moves to `apps/clawx-gui/src/lib/` (store, chat-store, rest, socket).
- `useEffect` dependency arrays exhaustive. If you disable the lint rule, add a one-line comment explaining why.
- No uncontrolled components for anything user-typed (chat input, token paste).
- Never mutate state — always produce new references. The `chat-store.ts` reducer pattern is the model.

## State

- `ClawProvider` / `useClaw()` is the single source of truth for WS session + token. Don't duplicate in component state.
- `ChatStore` owns message list + typing state. Callers subscribe via `.subscribe()` — no direct mutation.
- `localStorage["clawx.dashboard_token"]` is the token persistence key. Don't rename; users have tokens stored here.

## Wire Protocol

- Message type string literals (`"message.send"`, `"message.create"`, `"typing.start"`, etc.) are the wire contract — they must match `backend/hermes_bridge/ws/protocol.py` exactly. Any change touches both sides.
- Payload field names (`message_id`, `content`, `thought`, `request_id`, `session_id`) are equally fixed.
- `HermesMessage<P>` generic payload: prefer defining a specific payload interface over `Record<string, unknown>` when you know the shape.
- Never send tokens in URLs or query strings. Subprotocol for WS, `Authorization` header for REST.

## Networking

- All backend calls go through `hermes-rest.ts`. Don't `fetch()` from components directly.
- All WS connections go through `HermesSocket`. Reconnect + queue behavior is there for a reason.
- Error responses throw `HermesApiError` — callers check `instanceof` and render.

## Tests

- vitest + `@testing-library/react` + `jsdom`. Co-located under `__tests__/`.
- Mock at the `hermes-rest.ts` / `hermes-socket.ts` boundary, not at `fetch()` / `WebSocket` (except for tests of those modules themselves).
- `vi.hoisted()` for mocks that need to be set up before import.
- Always wrap async state updates in `act(async () => …)`.

## Imports

- Absolute imports via `../lib/…` are fine. Prefer these over deeply-relative (`../../../`).
- No barrel files (`index.ts` re-exporting). Named imports directly from the module.
- Imports grouped: external packages → project (`../lib/…`) → stylesheets. Blank line between groups.

## File Organization

- One component per file, same-name as the file.
- Hooks live alongside their consumers unless shared across pages — then `lib/`.
- No new top-level folders in `src/` without discussion. Current: `components/`, `pages/`, `lib/`.

## Accessibility

- All interactive elements keyboard-accessible (no click-only divs).
- Labels tied to inputs via `htmlFor` or wrapping `label`.
- `aria-label` for icon-only buttons.
- No `tabindex > 0`.

## i18n

- User-visible strings can be Chinese or English (both languages coexist in the codebase). Don't mix within a single sentence.
- No runtime translation framework unless the user asks — keep strings inline for now.
