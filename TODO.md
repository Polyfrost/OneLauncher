# todo list

- migrate launcher backend storage to `prisma-client-rust` (see `prisma` branch).
- migrate api commands that currently use `tauri-specta` but don't actually rely on `tauri` to `rspc`
 - waiting on `rspc` to update to the latest version (hopefully once tauri comes out of rc)
- bump `tauri` to stable v2 when it releases.
- test all major version/loader combos launching and mod loading.
- legacy fabric, babric, nilloader, java agent support
 - nilloader <https://github.com/modrinth/labrinth/issues/903> <https://github.com/orgs/modrinth/discussions/45>
- shared cluster resources and options (<https://github.com/enjarai/shared-resources>)
- ftb, technic, and tlauncher importing
- better debug logging and mc logging, for the launcher and spawn
- implement forgewrapper instead of processing ourselves
- allow per-cluster sandboxing
- implement gamemode
- better toml support
- make all tauri events into tauri_specta events
