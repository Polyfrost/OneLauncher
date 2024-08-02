# todo list

- migrate launcher backend storage to `prisma-client-rust` (see `prisma` branch).
- migrate api commands that currently use `tauri-specta` but don't actually rely on `tauri` to `rspc`
 - waiting on `rspc` to update to the latest version (hopefully before tauri comes out of beta)
- bump `tauri` to stable v2 when it releases.
- test all major version/loader combos launching and mod loading.
