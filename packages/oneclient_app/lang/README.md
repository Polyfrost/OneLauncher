# Launcher translations

`en_us.json` is the source language. `zh_tw.json` contains Traditional Chinese
for Taiwan (`zh_TW`) and is embedded in the application.

Choose **Settings > Language > 繁體中文**, or select it during initial setup.
Traditional Chinese is available without a network connection or a cached
translation manifest. The saved language is restored on the next launch.

Community languages are downloaded from the Crowdin distribution. If a cached
Traditional Chinese translation is available, its valid entries override the
bundled entries; missing or invalid entries keep the bundled translation.
Other languages retain their existing English fallback.

## Editing Traditional Chinese

- Keep the keys in both JSON files identical and in the same order.
- Preserve every format argument. `%s` fills arguments in order; `%1$s`, `%2$s`
  and so on can reorder them for Chinese grammar. `%%` produces a literal `%`.
- Keep line breaks and product names, file extensions, paths and commands intact.
- Use Taiwan terminology consistently: 模組, 模組包, 實例, 資料夾 and 伺服器.
- Add new interface text through `crate::i18n::{t, tf}` and update both files.

Run the language and loader tests from the repository root:

```sh
cargo test -p oneclient_app i18n::tests
cargo test -p oneclient_app translations::tests
```

These tests check source keys, complete Traditional Chinese coverage, placeholder
identity and counts, offline availability, and cached translation overlays.

Provider descriptions, mod names, downloaded changelogs, legal documents and
game-generated logs come from external sources. They are not translated by
these language files. Developer debug tools and the currently disabled feedback
page also retain their existing text.
