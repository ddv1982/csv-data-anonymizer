# Electrobun Migration Research Plan

Research date: 2026-06-18

Target: rewrite CSV Anonymizer from Electron/electron-vite/electron-builder to the latest stable Electrobun release.

## Research Summary

Latest stable Electrobun is `1.18.1`. This was verified from the npm registry on 2026-06-18 with `npm view electrobun version dist-tags time --json`; npm `latest` is `1.18.1`, while `beta` is `1.18.4-beta.6`. Do not base the rewrite on beta-only APIs unless that is an explicit product decision.

The shipped `electrobun@1.18.1` package reports:

- Bundled Bun: `1.3.13`
- Bundled CEF: `147.0.10+gd58e84d`
- Bundled Chromium: `147.0.7727.118`

The migration should not rewrite the CSV anonymization engine. The framework-specific surface is concentrated in:

- `package.json`: Electron scripts, dependencies, and `electron-builder` config.
- `src/main/index.ts`: Electron window, lifecycle, paths, shell, and window-state code.
- `src/main/ipc.ts`: Electron IPC handlers, dialogs, shell reveal.
- `src/preload/index.ts`: `contextBridge` / `ipcRenderer` bridge.
- `src/renderer/src/lib/api.ts` and `src/renderer/src/vite-env.d.ts`: renderer assumption that `window.csvAnonymizer` exists.
- `e2e/anonymizer.spec.ts` and `scripts/packaged-smoke.mjs`: Playwright Electron launch harness.
- release docs/workflows/scripts: electron-builder artifacts, macOS signing, Linux AppImage/deb/rpm/APT flow.

Portable code to keep and retest:

- `src/shared/contracts.ts`
- `src/core/*`
- `src/strategies/*`
- most of `src/main/services/anonymizerService.ts`
- most of `src/main/services/settingsStore.ts`
- most Vue components and composables under `src/renderer/src`

Manager-side verification after research:

- `pnpm run typecheck` passed.
- `pnpm test` passed: 21 files, 275 tests.

## Source-Backed Electrobun Facts

- Electrobun apps use a Bun main process imported from `electrobun/bun`, plus browser-side APIs from `electrobun/view`.
- Main windows are created with `new BrowserWindow({ title, frame, url, rpc })`, usually loading local bundled assets through `views://...`.
- Renderer-to-main communication is typed RPC, not Electron IPC. Bun-side handlers use `BrowserView.defineRPC`; browser-side code uses `Electroview.defineRPC` and `new Electroview({ rpc })`.
- `BrowserWindow` supports `frame`, `getFrame`, `setFrame`, `move`/`resize` events, `show`, `showInactive`, `activate`, `hide`, `minimize`, `isMinimized`, `setFullScreen`, and `isFullScreen`.
- Electrobun `sandbox: true` disables RPC. The main app view cannot use sandbox mode if it needs typed RPC.
- `Utils.paths.userData` is the closest equivalent for Electron `app.getPath('userData')`.
- `Utils.openExternal` and `Utils.showItemInFolder` map directly to the current shell use cases.
- `Utils.openFileDialog` exists for file/folder choosing. I found no official `showSaveDialog` equivalent in the docs or in the shipped `1.18.1` TypeScript API.
- `Screen.getAllDisplays()` and `Screen.getPrimaryDisplay()` exist, but there is no direct `screen.getDisplayMatching(bounds)` equivalent.
- Electrobun CLI docs and `1.18.1` CLI source both indicate builds target the current host OS/architecture. The config type contains a `targets` field, but the `1.18.1` CLI `runBuild` path uses only host `OS`/`ARCH`.
- Non-dev builds generate an `artifacts/` folder with flat `{channel}-{os}-{arch}-...` names, update metadata, `.tar.zst` update bundles, and platform install artifacts. This differs from `electron-builder`'s `release/${version}` output.
- Official docs do not show first-class `.deb`, `.rpm`, or AppImage package generation in Electrobun. Linux distribution is documented around `.tar.gz` setup artifacts and updater artifacts. The package source still contains older AppImage naming helpers, but the CLI source comments say Linux setup moved away from AppImage to avoid `libfuse2`.

Primary sources:

- https://docs.electrobunny.ai/electrobun/
- https://docs.electrobunny.ai/electrobun/apis/cli/cli-args/
- https://docs.electrobunny.ai/electrobun/apis/cli/build-configuration/
- https://docs.electrobunny.ai/electrobun/apis/browser-window/
- https://docs.electrobunny.ai/electrobun/apis/browser-view/
- https://docs.electrobunny.ai/electrobun/apis/browser/electroview-class/
- https://docs.electrobunny.ai/electrobun/apis/utils/
- https://docs.electrobunny.ai/electrobun/apis/events/
- https://docs.electrobunny.ai/electrobun/guides/bundling-and-distribution/
- https://docs.electrobunny.ai/electrobun/guides/cross-platform-development/
- https://docs.electrobunny.ai/electrobun/guides/code-signing/
- https://docs.electrobunny.ai/electrobun/guides/changelog/v1-18-0/
- npm package tarball: `electrobun@1.18.1`

## Recommended Strategy

Use an incremental rewrite, not a greenfield rewrite.

The safest first milestone is an Electrobun shell that preserves the existing renderer API contract and CSV behavior:

- Keep the renderer-facing `CsvAnonymizerApi` contract.
- Keep or emulate `window.csvAnonymizer` so most renderer code remains unchanged.
- Keep Vite as the Vue/Tailwind renderer build initially.
- Replace Electron IPC with Electrobun typed RPC behind the same high-level methods.
- Replace `electron-builder` with Electrobun packaging only after dev/build/runtime behavior is stable.
- Treat Linux `.deb`/`.rpm`/APT preservation as a separate packaging milestone, not a prerequisite for the first Electrobun app boot.

## Migration Plan

### Phase 0 - Product And Release Decisions

Decide these before implementation:

1. Stable version: target `electrobun@1.18.1`.
2. Package manager: keep `pnpm` initially for lockfile stability, but install/setup Bun in dev and CI because Electrobun runs through Bun. Run a disposable `bun install` migration later before deciding whether to commit `bun.lock`.
3. Renderer path: keep Vite initially and package its static output through Electrobun. Spike direct Bun.build Vue/Tailwind only after the shell works.
4. Linux distribution: decide whether current APT/deb/rpm install/update contract must remain. If yes, add a custom packaging milestone around Electrobun Linux output.
5. Linux renderer: strongly consider `build.linux.bundleCEF: true` for distribution consistency; default WebKitGTK is smaller but more dependent on distro packages and has documented limitations.
6. Save dialog UX: choose a replacement for Electron `dialog.showSaveDialog`, since Electrobun stable does not appear to ship one. Options: keep manual output-path input, use directory chooser plus generated filename, or contribute/build a native save-dialog bridge.

### Phase 1 - Add Electrobun Shell Without Touching CSV Core

Create new framework files while keeping Electron files until the Electrobun path is proven:

- Add `electrobun.config.ts`.
- Add `src/bun/index.ts` as the Electrobun main process.
- Add `src/bun/rpc.ts` for typed RPC handlers.
- Add `src/electrobun-view` or similar browser bootstrap that creates the `Electroview` RPC client and exposes `window.csvAnonymizer`.
- Add an Electrobun-specific HTML entry or Vite output copy path that loads through `views://mainview/index.html`.

Expected config shape:

- `app.name`: `CSV Anonymizer`
- `app.identifier`: keep or intentionally change from `com.csv-anonymizer.app`
- `app.version`: read from `package.json` or set from package version in config
- `runtime.exitOnLastWindowClosed`: likely `true`, then separately test macOS dock activation expectations
- `build.bun.entrypoint`: `src/bun/index.ts`
- `build.copy`: copy renderer HTML/assets into `views/mainview/...`
- platform icons and CEF settings

Verification gate:

- `electrobun build --env=dev`
- `electrobun run`
- The main window opens the Vue UI from `views://...`

### Phase 2 - Replace Electron Main Process Behavior

Port behavior from `src/main/index.ts`, not the implementation.

Map:

- `BrowserWindow` -> `electrobun/bun` `BrowserWindow`
- `loadFile` / dev `loadURL` -> `views://mainview/index.html` and, if needed, separate dev renderer strategy
- `app.getPath('userData')` -> `Utils.paths.userData`
- `app.getVersion()` -> app version from config/package or `Updater.getLocalInfo()` after verifying dev behavior
- `shell.openExternal` -> `Utils.openExternal`
- `shell.showItemInFolder` -> `Utils.showItemInFolder`
- `screen.getDisplayMatching` -> custom best-display calculation using `Screen.getAllDisplays()`
- `ready-to-show` -> `dom-ready` / explicit show strategy; no exact equivalent found
- `minWidth` / `minHeight` -> custom resize enforcement or accepted gap; no stable documented equivalent found

Keep behavior:

- Persist `window-state.json`.
- Clamp restored bounds onto a visible display.
- Avoid saving minimized/fullscreen state.
- Allow external `https`, `http`, and `mailto` only.

Verification gate:

- Window opens with expected size.
- Window state persists across restarts.
- External links open outside the app only for allowed protocols.
- Settings file persists under the new Electrobun user-data path.

### Phase 3 - Replace IPC With Typed RPC

Current Electron channels:

- `app:health`
- `settings:get`
- `settings:update`
- `dialog:select-csv`
- `dialog:select-output`
- `shell:show-output`
- `csv:headers`
- `csv:preview`
- `csv:anonymize`

Create an Electrobun RPC schema that mirrors `CsvAnonymizerApi`.

Bun-side:

- Instantiate `AnonymizerService` and `SettingsStore`.
- Define request handlers with `BrowserView.defineRPC`.
- Reuse Zod validation and `result()` / `toApiFailure()` behavior from `src/main/ipc.ts`.

Browser-side:

- Create `Electroview.defineRPC`.
- Expose a wrapper as `window.csvAnonymizer` to minimize renderer changes.
- Keep `src/renderer/src/lib/api.ts` mostly unchanged if possible.

Dialog handling:

- `selectCsvFile`: use `Utils.openFileDialog` with CSV filters and adapt return shape to `{ filePath }`.
- `selectOutputFile`: unresolved native save dialog. Implement a temporary product-safe fallback before deleting Electron.

Verification gate:

- All unit/integration tests around service behavior still pass.
- Renderer can call all API methods.
- CSV preview and anonymize work through RPC.
- Invalid payloads still become structured `ApiFailure` responses.

### Phase 4 - Renderer Build

Recommended first implementation:

- Keep Vite, Vue SFC, Tailwind, PostCSS, and current renderer source.
- Configure Vite with a relative/static base suitable for `views://`.
- Build renderer output to a deterministic intermediate directory.
- Copy the built HTML/assets into Electrobun `views/mainview/...`.

Why:

- Electrobun docs say `build.views` accepts Bun.build options, but they do not document a first-party Vue SFC/Tailwind path.
- Keeping Vite reduces migration risk and isolates framework replacement from frontend bundling changes.

Later optimization:

- Spike direct `build.views` Vue/Tailwind support with Bun plugins.
- Only replace Vite if the result is simpler and equally testable.

Verification gate:

- UI renders identically enough under native renderer and CEF.
- Tailwind styles load from `views://`.
- Browser console has no asset-load errors.

### Phase 5 - Tests

Keep:

- Vitest unit and integration tests.
- Core/strategy/service tests.
- Renderer component/composable tests.

Rewrite:

- `tests/unit/ipc.test.ts` into RPC handler tests.
- `e2e/anonymizer.spec.ts`, because Playwright `_electron.launch` cannot launch Electrobun.
- `scripts/packaged-smoke.mjs`, because it launches Electron packaged apps.

E2E options to evaluate:

- A custom Electrobun test mode that exposes a controlled RPC/evaluate hook.
- Launch app as a child process and test through an app-internal automation route.
- Community tools such as `electrobun-browser-tools`, `electrobun-e2e`, or `bun-mot`, after source review.
- Minimal packaged smoke checks first: app starts, bridge health responds, CSV preview/anonymize works, app exits cleanly.

Verification gate:

- `pnpm run typecheck`
- `pnpm test`
- Electrobun dev smoke
- Electrobun packaged smoke
- Platform-specific smoke on macOS and Linux before release flow migration

### Phase 6 - Packaging And CI

Replace package scripts in stages.

Initial scripts:

- `dev`: run Electrobun dev path
- `build`: typecheck, build renderer, then `electrobun build --env=dev`
- `build:stable`: typecheck, build renderer, then `electrobun build --env=stable`
- `build:canary`: same with `--env=canary`
- keep `test`, `typecheck`, and coverage scripts

CI changes:

- Install Bun on runners.
- Keep pnpm initially for dependencies and tests unless a Bun lockfile migration is accepted.
- Replace Electron dependency install steps with Electrobun/Linux webview or CEF prerequisites.
- Use one native runner per target OS/architecture.
- Do not assume a Linux runner can produce macOS or Windows release artifacts.

macOS:

- Replace electron-builder signing with Electrobun `build.mac.codesign` and `build.mac.notarize`.
- Replace current Electron entitlements with Electrobun/Bun-appropriate entitlements.
- Use Electrobun env vars: `ELECTROBUN_DEVELOPER_ID`, `ELECTROBUN_TEAMID`, and either Apple ID password vars or App Store Connect API key vars.
- Keep verification with `codesign`, `stapler`, and `spctl`.

Linux:

- Electrobun stable output should be validated first as documented `.tar.gz`/`.tar.zst` artifacts.
- If preserving `.deb`/`.rpm`/APT, add a custom packaging layer that wraps the Electrobun Linux app output and preserves:
  - `/usr/share/applications/csv-anonymizer.desktop`
  - `/usr/share/metainfo/io.github.ddv1982.csv-data-anonymizer.metainfo.xml`
  - Debian copyright
  - RPM license path
  - current executable name and package name
- Reuse or adapt current `scripts/build_apt_repository.py` only if `.deb` output remains.

Windows:

- Existing config lists Windows targets, but release docs/workflows do not currently publish Windows. Decide if Windows is in scope before spending migration effort.

### Phase 7 - Release Cutover

Do not delete the Electron release flow until Electrobun packaging is accepted.

Suggested cutover:

1. Land Electrobun dev/runtime path behind new scripts.
2. Land Electrobun packaged smoke for at least macOS and Linux.
3. Produce test Electrobun artifacts from CI.
4. Decide Linux distribution model.
5. Update docs/releasing.md and release workflows.
6. Remove Electron dependencies/config/scripts after release path parity is approved.

## Risk Register

| Risk | Severity | Plan |
| --- | --- | --- |
| No official save dialog in Electrobun `1.18.1` | High | Keep manual output path, use folder picker plus generated filename, or build/contribute save-dialog bridge. |
| Electrobun sandbox disables RPC | High | Do not use sandbox on trusted local app view; use URL rules and keep remote content sandboxed. |
| Linux `.deb`/`.rpm`/APT contract not supported directly | High | Decide whether to preserve APT; if yes, build custom packaging around Electrobun output. |
| Vue/Tailwind through Bun.build undocumented | Medium | Keep Vite renderer build first. |
| Playwright Electron E2E does not transfer | Medium | Build new smoke harness before deleting Electron path. |
| `minWidth`/`minHeight` direct equivalent not found | Medium | Implement resize enforcement or accept temporary behavior difference. |
| macOS activation behavior unclear | Medium | Platform smoke test dock reopen/last-window-close lifecycle. |
| Native renderer differences | Medium | Prefer CEF on Linux distribution if artifact size is acceptable. |
| pnpm-to-Bun package manager migration can lose pnpm workspace settings | Medium | Keep pnpm first; inspect Bun migration in disposable copy before committing. |

## Go / No-Go Milestones

Go to implementation when these are accepted:

- Target `electrobun@1.18.1`.
- Initial renderer strategy is Vite output packaged into Electrobun.
- A temporary save-output UX is acceptable until native save dialog support is resolved.
- Linux distribution path is explicitly chosen: Electrobun artifacts only, or custom packages to preserve APT.

First implementation milestone is complete when:

- Electrobun dev build opens the current Vue app.
- `window.csvAnonymizer` wrapper works over Electrobun RPC.
- CSV headers, preview, anonymize, settings, and show-output behaviors work.
- `pnpm run typecheck` and `pnpm test` pass.
- A minimal Electrobun smoke test exists.

Release migration is complete when:

- macOS stable artifacts are signed/notarized and verified.
- Linux artifacts are accepted under the chosen distribution model.
- Release docs and GitHub Actions no longer assume electron-builder output.
- Electron dependencies and configs are removed.

## Implementation Status - 2026-06-18

- Phase 0: Implemented for the initial migration path. The project targets `electrobun@1.18.1`, keeps `pnpm`, keeps Vite for renderer output, uses Linux CEF, and uses a directory-picker output fallback until native save-dialog support is available.
- Phase 1: Implemented. The Electrobun config, Bun main process, typed RPC handlers, browser RPC bridge, and `views://mainview/index.html` renderer copy path are present.
- Phase 2: Implemented for the core runtime path. Window state, user-data path mapping, display clamping, minimum-size enforcement, allowed external URL handling, and shell reveal behavior are ported.
- Phase 3: Implemented. All nine renderer-facing API methods are available through Electrobun typed RPC while preserving `window.csvAnonymizer`.
- Phase 4: Implemented. Vite builds the Vue/Tailwind renderer with a relative static base into `dist/renderer`, and Electrobun copies it into `views/mainview`.
- Phase 5: Implemented. Unit/integration tests remain, RPC handler tests were added, and `smoke:electrobun` exercises app startup plus health, headers, preview, anonymize, and output validation through an app-internal Electrobun smoke route. The legacy Playwright Electron E2E harness has been removed.
- Phase 6: Implemented. CI installs Bun, builds Electrobun, runs the Electrobun smoke workflow, validates Electrobun artifacts, builds `.deb`, `.rpm`, and AppImage package-manager wrappers on Linux, validates package metadata, and checks APT repository generation.
- Phase 7: Implemented for signed release platforms. The release workflow builds signed/notarized macOS Electrobun artifacts on native macOS runners, builds Linux Electrobun artifacts plus package-manager wrappers and a signed APT repository on Ubuntu, deploys the APT repository to GitHub Pages, uploads release assets, and publishes only after macOS, Linux, and APT jobs succeed.

Remaining external release requirements:

- macOS signing and notarization require the documented Apple certificate and App Store Connect secrets.
- Linux signatures and APT publishing require the documented GPG signing secrets and GitHub Pages configured for Actions deployments.
- Windows release artifacts are deferred until Authenticode signing is configured.
- A real tag-triggered GitHub release run is still required to verify hosted macOS, Linux, and Pages publishing behavior end to end.
