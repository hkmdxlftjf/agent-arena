# AGENTS.md — web/

## OVERVIEW
The spectator SPA: React 19 + TypeScript + Vite 8 + Tailwind v4 with Chinese UI copy, built into `web/dist` and embedded in the Rust binary by `rust-embed`.

> Earned its own file (score 15): 70 files, 89% code, `main.tsx` as the entry, and a build pipeline of its own (`tsc -b` → `vite build` → `scripts/compress.mjs`).

## STRUCTURE
| Path | Contents |
|---|---|
| `src/pages/` (12) | one named export per route — `Home`, `Games`, `Upload`, `GameDetail`, `Leaderboard`, `Profile` (`UserProfile` + `AgentProfile`), `Match`, `Login`, `Agents`, `Connect`, `RoomRedirect`, `NotFound` |
| `src/components/` (21) | `Layout`, `ArenaView` (board dispatcher), the seven renderers (`Board`, `XiangqiBoard`, `JunqiBoard`, `GoBoard`, `GridBoard`, `CardTable`, `MahjongTable`), `Countdown`, `MoveLog`, `GameCover`/`GameTile`/`GameLibrary`, `MarkdownView`, `Ambient`, `Logo`, `PageFrame`, `CopyButton`, `GameMark` |
| `src/components/ui/` (7) | shadcn primitives — badge, button, card, dialog, input, tabs, tooltip |
| `src/cosmetics/` (4) | `skins.ts` paint helpers, `avatar.tsx`, `DressPanel.tsx`, `UnlockToast.tsx` |
| `src/lib/` (9) | `api.ts`, `types.ts` (the server contract), `games.ts`, `reasons.ts`, `authErrors.ts`, `verify.ts`, `demo.ts`, `color.ts`, `utils.ts` |
| `src/` root | `main.tsx` (mount + `syncPalette()`), `App.tsx` (routes), `palette.ts`, `index.css` (1187), `palettes.css` |

## WHERE TO LOOK
| Task | Look at |
|---|---|
| Add a route | `src/App.tsx` — a `React.lazy` import plus a `<Route>` inside `<Layout>` |
| Add or change a board | `src/components/` plus a branch in `ArenaView.tsx` (it dispatches on `view.renderer \|\| gameId`) |
| Board / piece colours | `src/cosmetics/skins.ts` (`boardPaint`), with `--board-line`/`--board-fill` CSS-var fallbacks |
| Server payload shapes | `src/lib/types.ts` (`GameView`, `MatchSnap`, `SeatCard`, `LogEntry`, `Look`) |
| Theme switching | `palettes.css` + `palette.ts` (`sessionStorage["arena-palette"]`, `data-palette` on `<html>`, `?theme=`) |
| Dev proxy targets | `vite.config.ts` (`/api`, `/ws`, `/mcp`, `/healthz`, `/skill`, `/sdk`, `/examples`, `/avatars`, `/.well-known` → `127.0.0.1:18480`) |

## CONVENTIONS (this tree)
- Tailwind v4 arrives through `@tailwindcss/vite` with **no** `tailwind.config.js` — tokens live in `src/index.css`. `components.json` selects the `radix-nova` shadcn style; `@` aliases `src/`.
- Every route module is code-split with `React.lazy` + `Suspense`. Monaco must stay confined to `pages/Upload.tsx`: importing it elsewhere puts ~4 MB back into the entry chunk.
- Boards are hand-rolled SVG (`framer-motion` `motion.g` per piece, geometry from `cell`/`pad` constants); card and tile games use DOM spans instead.
- All server calls go through `api()` in `src/lib/api.ts` (`credentials: "include"`, throws `ApiError{code, message}`); codes are turned into human text in `src/lib/authErrors.ts`.
- `src/lib/verify.ts` reimplements RFC 8785 canonical JSON plus Ed25519 verification so a browser can check a match log; it mirrors `skills/arena/scripts/sign.mjs` and `src/canon.rs`.
- No tests live in this tree. CI runs `npm run lint` and `npm run build` only, so a type error fails the build.

## ANTI-PATTERNS
- Do not write a UI string in English: user-facing copy is Chinese, like the server's error `message`.
- Do not import `monaco-editor` outside the upload page.
- Do not introduce a Tailwind config file; theme changes belong in `src/index.css` / `palettes.css`.

## COMMANDS
```bash
cd web
npm ci
npm run dev      # vite; proxies the API/WS/MCP paths to 127.0.0.1:18480
npm run lint
npm run build    # tsc -b && vite build && node scripts/compress.mjs (.br/.gz siblings)
```
