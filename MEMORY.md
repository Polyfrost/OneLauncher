# RemixClient — project memory

This file is the single place for decisions, feature status, placeholders and the
next steps. Update it whenever a feature lands or a placeholder is replaced.

- Project: **RemixClient** — offline-first community fork of
  [OneLauncher](https://github.com/Polyfrost/OneLauncher) by Polyfrost.
- License: **GPL-3.0-only**. Keep `LICENSE`, `ATTRIBUTION.md` and upstream
  copyright notices. This is an unofficial fork, not affiliated with Polyfrost,
  Mojang or Microsoft.
- Scope decision: rebrand only the user-facing product (brand, UI, installer,
  data folders). Internal crate names stay `oneclient_*` on purpose.

## Current status

| Area | Status | Notes |
|---|---|---|
| Offline accounts | done | Nickname + local UUID (`MD5("OfflinePlayer:<name>")`), no Microsoft call, `--userType legacy` |
| Microsoft sign-in | optional | Uses `REMIXCLIENT_MICROSOFT_CLIENT_ID`, fallback is the upstream app id — replace before public release |
| Portable mode | done | `--portable` / `-p`, `portable.txt` marker, `RemixClient-data` next to the exe, separate IPC |
| World backups | done | Auto before launch (changed worlds only, retention 5), manual backup + restore from Worlds → right-click → Backups… |
| Offline skins/capes | done (needs in-game test) | Local HTTP texture server + `--userProperties`, shared dialog in Accounts and Wardrobe |
| Crash autopsy | done | Suspected mods get a one-click **Disable** button |
| Discord RPC | done | App ID `1569698419491217500`; Discord desktop must be running; large image optional |
| Russian | done | `lang/ru_RU.json`, 100% of keys; machine-translated + curated, proofreading welcome |
| Help tab | done | Settings → Help, feature guide + report/logs buttons |
| Report button | placeholder | `ISSUES_URL` in `packages/oneclient_app/src/view/app/settings/help.rs` points to `https://github.com/RemixClient/RemixClient/issues/new/choose`; replace when the real repo exists |
| MSI / release | blocked | Release binary builds locally; MSI not built after the latest features; waiting for GitHub repo |
| CI / GitHub Actions | todo | fmt, clippy, tests, MSI, release signing |
| Code signing (Authenticode) | skipped | User chose to skip; SmartScreen will warn |
| ToS / Privacy | todo | Onboarding still loads Polyfrost terms from `data-v2.polyfrost.org`; replace with own notice or keep as upstream-service disclosure |
| Sentry | disabled | No DSN by default; `REMIXCLIENT_SENTRY_DSN` enables it |
| CurseForge | works | Embedded upstream key; override with `REMIXCLIENT_CURSEFORGE_API_KEY` |
| Updater | done (endpoint pending) | Endpoint `https://github.com/RemixClient/RemixClient/releases/latest/download/latest.json`; new minisign key pair generated |
| Linux/mac packaging | partial | Display names updated; IDs/files still `org.polyfrost.*`; macOS `OneClient.icon` assets remain |
| App icon | partial | Current generated R icon; user has an AI icon that still needs to be saved to a file and converted |
| HUD (axe/sword/totem + presets) | research | In-game HUD needs a mod; subagent researching existing mods vs companion mod |
| QA sandbox | research | Subagent researching Windows Sandbox / separate desktop / UI automation |

## Keys, secrets and endpoints

- Discord Application ID: `1569698419491217500` (`oneclient_common/src/constants.rs`).
- Discord large image: optional `REMIXCLIENT_DISCORD_LARGE_IMAGE`; without it Discord
  shows the application icon uploaded in the Developer Portal.
- Updater private key: `C:\Users\user\remixclient-updater.key` (never commit, back it up).
  Public key is embedded in `packages/oneclient_app/src/constants.rs`.
- Updater releases URL: `https://github.com/RemixClient/RemixClient/releases/latest`.
- Microsoft app id override: `REMIXCLIENT_MICROSOFT_CLIENT_ID`.
- CurseForge key override: `REMIXCLIENT_CURSEFORGE_API_KEY`.
- Sentry: `REMIXCLIENT_SENTRY_DSN` (empty by default).
- Portable flag: `REMIXCLIENT_PORTABLE=1` also works.
- Discord RPC only works while the Discord desktop client is running and logged in.
  There is no server component.

## Build and test

Run commands through the MSVC wrapper so `link.exe` resolves to Visual Studio:

```powershell
C:\Users\user\msvc.cmd cargo check -p oneclient_app
C:\Users\user\msvc.cmd cargo test -p oneclient_app --lib
C:\Users\user\msvc.cmd cargo test -p oneclient_auth
C:\Users\user\msvc.cmd cargo test -p oneclient_mc -j 1
C:\Users\user\msvc.cmd cargo test -p oneclient_common
C:\Users\user\msvc.cmd cargo test -p oneclient_core --lib world_backup -j 1
C:\Users\user\msvc.cmd cargo test -p oneclient_core --lib offline_skin -j 1
```

- `SQLX_OFFLINE=true` is set by the wrapper.
- Full `cargo test -p oneclient_core` may fail on this machine because of a small
  Windows pagefile (`os error 1455`) while linking integration tests. Use `--lib`
  and `-j 1`.
- Release build: `C:\Users\user\msvc.cmd cargo build --release -p oneclient_app`.
- MSI (when ready): `cargo packager -p oneclient_app --release -f wix`.
  `cargo-packager` does not build the binary itself; build release first.

## Data folders

| Build | Folder |
|---|---|
| Release | `%APPDATA%\Remix\RemixClient` |
| Debug | `%APPDATA%\Remix\RemixClient-dev` |
| Portable | `<exe dir>\RemixClient-data` |

First launch migrates `%APPDATA%\Polyfrost\OneClient(-dev)` when the new folder is
missing. World backups live in `<instance>/backups/worlds/<world>/*.zip`. Offline
skins live in `<data>/skins/<account uuid>/`.

## Subagent policy (hard rule)

- **Never** launch a subagent on a paid model. Always use a free / flash-tier model.
- Default free picks: `deepai/qwen-3.8-flash`, `deepai/deepseek-v4.1-flash`,
  `deepai/glm-5.3-flash`, `deepai/mimo-2.6-flash`, `deepai/minimax-m3.1-flash`,
  `deepai/solar-mini-4`.
- Never delegate to Claude / Opus / GPT / "-max" / "-pro" / "prime" models.
- Paid-tier subagents burned the daily limit on 2026-11-10 and produced no reports.
  The main thread may use whatever model the user picked; only children are restricted.

## Decisions

- Keep Polyfrost **content** services (bundles, metadata, translations, Poly+) for
  compatibility, but self-updates point to the RemixClient GitHub repo so an update
  cannot turn the fork back into OneClient.
- Sentry is off by default; Discord RPC is opt-out via settings.
- Offline accounts never touch Microsoft. Microsoft sign-in is optional.
- World backups are per instance and skip unchanged worlds; restore always creates a
  safety backup first.
- Offline skin textures are served from a loopback HTTP server for the launcher
  lifetime; the game receives a standard `textures` profile property.
- HUD/cosmetics that draw over Minecraft are **not** a launcher feature; they need a
  mod. Do not add a fake launcher-only setting for them.

## Placeholders to replace

1. `ISSUES_URL` in Help → real GitHub repository URL.
2. Updater endpoint → real repository releases (`latest.json` + signed artifacts).
3. Discord large image key → upload `remixclient_logo` in the Developer Portal and set
   `REMIXCLIENT_DISCORD_LARGE_IMAGE`.
4. App icon → AI-generated PNG → regenerate PNG/ICO/ICNS.
5. ToS/Privacy → own notice or explicit upstream-service disclosure.
6. Microsoft/CurseForge keys → own values for public builds.
7. `org.polyfrost.*` identifiers in Linux/mac packaging.
8. MSI + code signing decision.

## Known issues / reminders

- Discord RPC requires the Discord desktop app open; closing it removes the status.
- Offline skins still need a real-game verification on the user's target version.
- World backup zips currently read each file into memory; very large worlds can be slow
  and the 180 s timeout may skip them.
- Update check fails until the GitHub repository exists (expected).
- Russian is machine-translated in places; review the main screens.
- Report button is a placeholder until the repository is public.

## User decisions (2026-11-10)

- **GitHub / release infra**: the user will create the repository; MSI, CI and signing come after that.
- **ToS/Privacy**: not understood / postponed. Keep as backlog; the current onboarding still shows Polyfrost terms.
- **Keys**: Discord is enough for now. CurseForge users can attach their own key. Microsoft sign-in is optional.
- **LAN hosting, mod health, instance snapshots, low-end profile**: not priorities right now; keep in backlog. World backups already cover the critical data-loss case.
- **Report button**: placeholder GitHub Issues URL is acceptable until the repository is public; this file tracks the replacement.
- **Memory**: this `MEMORY.md` is the source of truth for features, placeholders and decisions.
- **QA sandbox**: the user wants a separate desktop/environment where the agent can run RemixClient, click through all settings and report broken/unneeded features. The agent must ask before disabling anything.
- **HUD**: the user wants small in-game axe/sword/totem indicators with one-shot hit animations, 3 popular presets and saved custom presets. This is a mod/client feature, not launcher-only; research in progress.
- **Visuals**: add later; do not fake a launcher-only HUD without a mod.

## Open questions

- Which Minecraft version and loader should the HUD mod target first (Fabric/Forge/NeoForge, 1.21.x / 1.8.9)?
- Does the user want a companion HUD mod written from scratch, or an existing mod bundled with launcher presets?
- Is a separate Windows Sandbox/VM acceptable for QA, or should QA use portable mode + UI automation in the current session?
- Should the fork keep Polyfrost content services long-term or move to self-hosted endpoints?

## Next steps

1. Collect the three research reports (HUD, QA sandbox, competitive features).
2. Integrate the user's AI icon and regenerate the launcher icon set.
3. Create the GitHub repository, CI, signed release, MSI.
4. Replace ToS/Privacy and the report URL.
5. Decide the HUD approach and implement the first preset (after user approval).
