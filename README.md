# RemixClient

**The improved, offline-first Minecraft launcher.** No licence, no Microsoft account, no telemetry required — just pick a nickname and play.

RemixClient is a community fork of [OneLauncher](https://github.com/Polyfrost/OneLauncher) by **Polyfrost**, rebranded and extended. It keeps the things that made the original great — fast instance management, mod and pack handling, automatic Java provisioning — and removes the parts that stopped people from playing: mandatory Microsoft sign-in and licence checks.

> **Unofficial fork.** RemixClient is **not** affiliated with, endorsed by, or supported by Polyfrost, Mojang Studios, Microsoft, or the OneLauncher project. All trademarks belong to their respective owners. See [Legal](#legal--attribution).

---

## Why RemixClient?

- **Offline by default.** Create a local account with any nickname. RemixClient generates the standard offline UUID (`MD5("OfflinePlayer:<name>")`) and launches the game with `--userType legacy`, so no Microsoft/Xbox/Minecraft API call is made.
- **No licence wall.** If you own the game, great. If you just want to play singleplayer, LAN or an offline-mode server, RemixClient does not ask for an account.
- **Your data stays yours.** Crash reporting is off unless a maintainer supplies their own Sentry DSN. No analytics, no fingerprinting.
- **Familiar and fast.** Instances, mods, packs, Java, worlds, logs and sharing work like the original launcher you already know.
- **Russian out of the box.** A bundled Russian translation ships with the binary and works without the translation CDN.
- **Still compatible.** Existing OneClient data, instances and accounts are imported automatically on first launch.

## Features

### Working today

- Offline accounts with locally generated UUIDs and no Microsoft login.
- Optional Microsoft sign-in for users who want social features.
- **Portable mode** — one folder, a USB stick, no installer and no AppData writes (`--portable`).
- **World backups before every launch**, with one-click rollback and a safety copy on restore.
- **Crash autopsy** — parse the log, point at the mod that broke the game, offer to disable it.
- **Offline skins and capes** — local skin files served from a loopback server, no Mojang session server.
- Instance manager with per-version settings (memory, JVM/game args, resolution, GPU, hooks, environment).
- Mod, resource pack, shader and datapack management.
- Modrinth and CurseForge browsing and updates.
- Automatic Java discovery and installation.
- World, log and screenshot views; crash reports and log sharing.
- Import from OneClient exports, Prism Launcher, MultiMC, PolyMC and the Modrinth App.
- Cloud sync and sharing through the upstream Polyfrost services (see [Updates & services](#updates--services)).
- Bundled English and Russian UI, plus an in-app help tab.

### Roadmap

1. **In-game HUD** — small axe/sword/totem indicators with durability, a one-shot hit animation and launcher-managed presets (needs a companion mod; no existing mod covers it).
2. **LAN world hosting without Microsoft** — open a world to the local network from the launcher.
3. **Optimised dedicated server** — a launcher-side server manager for offline-mode servers with sane defaults, Aikar flags and easy mod/plugin install.
4. **File-based profile sharing** — export/import a complete instance as one file, no proprietary cloud.
5. **Mod health checks** — duplicates, wrong loader, missing dependencies, known conflicts.
6. **Signed, reproducible releases** for Windows, Linux and macOS.
7. **More bundled translations**, maintained by the community in this repository.

## Install

### Windows (MSI)

Download the latest `RemixClient_<version>_x64_en-US.msi` from [Releases](https://github.com/linkure8888-lang/OneLauncher/releases/latest) and run it.

The installer registers `RemixClient` as a normal application, creates a Start Menu shortcut and can be removed from *Apps & Features*. The binary itself is still called `oneclient_app.exe` internally so that the fork stays compatible with the upstream build system; the product, shortcut and window title are `RemixClient`.

### Portable

Build or download `oneclient_app.exe` and run it from any folder. Game data still lives in the launcher data folder unless you change it in *Settings → Launcher → Launcher Folder*.

### Where data lives

| Build | Folder |
|---|---|
| Release | `%APPDATA%\Remix\RemixClient` |
| Debug / development | `%APPDATA%\Remix\RemixClient-dev` |

On first launch RemixClient tries to move the old upstream folder (`%APPDATA%\Polyfrost\OneClient` or `OneClient-dev`) to the new location. If the move fails (for example, a different drive), it keeps using the old folder instead of starting empty.

## Updates & services

RemixClient separates two things:

- **Launcher self-updates** come from this repository's GitHub Releases (`https://github.com/linkure8888-lang/OneLauncher/releases/latest/download/latest.json`) and are verified with the RemixClient minisign public key. Updating will never silently replace RemixClient with upstream OneClient.
- **Content services** — bundles, version metadata, translations CDN, sharing and Poly+ — still use the upstream Polyfrost endpoints, because that is the ecosystem this fork was built on. These are public services owned by Polyfrost and may change or be restricted at any time. The fork is not affiliated with them.

If you want a fully independent build, replace the endpoint constants in `packages/oneclient_common/src/constants.rs` and `packages/oneclient_app/src/constants.rs` with your own services.

## Build from source

### Requirements

- Rust **1.97+** (stable works; `rustup default stable`).
- **Visual Studio Build Tools** with the *Desktop development with C++* workload (MSVC linker + Windows SDK).
- Network access on the first build (crates and the `plusrenderer` git dependency).

### Commands (Windows PowerShell)

```powershell
git clone https://github.com/linkure8888-lang/OneLauncher.git
cd RemixClient

$env:SQLX_OFFLINE = "true"   # the repo ships .sqlx query data

cargo test -p oneclient_auth -p oneclient_mc
cargo run -p oneclient_app
```

If `link.exe` resolves to Git's `link.exe`, run the build from a *Developer PowerShell for VS* prompt or call `vcvars64.bat` first.

### Build the MSI

```powershell
cargo install cargo-packager --locked
$env:SQLX_OFFLINE = "true"
cargo packager -p oneclient_app --release -f wix
```

`cargo-packager` downloads the WiX toolset automatically. The MSI is written under `target/release/.cargo-packager/wix/`.

For faster local release builds you can override the expensive profile settings:

```powershell
$env:CARGO_PROFILE_RELEASE_LTO = "thin"
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16"
```

### Signing releases

Generate a minisign key pair once and keep the private key secret:

```powershell
cargo packager signer generate --path C:\keys\remixclient.key
```

Put the public key into `UPDATER_PUBKEY` in `packages/oneclient_app/src/constants.rs`, and sign release artifacts with `CARGO_PAGER_SIGN_PRIVATE_KEY` (see `cargo packager signer --help`). Never commit the private key.

## Project layout

```
packages/
  oneclient_app/       desktop UI (Freya), onboarding, settings, updater
  oneclient_auth/      Microsoft OAuth, offline accounts, credentials store
  oneclient_core/      launcher state, instances, Java, launch, sync, sharing
  oneclient_mc/        Minecraft metadata, assets, libraries, arguments, profiles
  oneclient_content/   mod providers, artifact store, bundles
  oneclient_cluster/   instance records, settings profiles, on-disk files
  oneclient_db/        SQLx database layer
  oneclient_java/      Java discovery and installation
  oneclient_net/       HTTP transport, retries, caching, verified downloads
  oneclient_polyplus/  Poly+ social and playtime client
  oneclient_common/    paths, constants, platform, shared types
  oneclient_events/    event bus (notifications, progress, prompts)
  oneclient_discord/   Discord Rich Presence
  oneclient_macro/     proc macros
  polyio/              async filesystem and archive helpers
```

## Contributing

Issues and pull requests are welcome. Good first contributions:

- Russian and other translations (`packages/oneclient_app/lang/`).
- Bug reports with a log from *Settings → Launcher → Open Launcher Folder → logs*.
- Testing on Linux and macOS.
- Feature work from the roadmap.

Please run `cargo fmt` and `cargo clippy` before opening a PR, and keep the GPL headers and attribution intact.

## Legal & attribution

- RemixClient is distributed under the **GNU General Public License v3.0 only**. See [LICENSE](LICENSE). Any distributed build must remain GPL-3.0 and include the corresponding source.
- RemixClient is a **modified fork** of [OneLauncher](https://github.com/Polyfrost/OneLauncher), Copyright Polyfrost and contributors. The original copyright notices, `LICENSE` and [ATTRIBUTION.md](ATTRIBUTION.md) are preserved.
- **Not affiliated** with Polyfrost, Mojang Studios or Microsoft. *Minecraft*, *Microsoft*, *Xbox*, *OneClient* and *Polyfrost* are trademarks of their respective owners. RemixClient does not claim ownership of them and does not imply endorsement.
- The launcher can launch Minecraft, but it does not grant a Minecraft licence. Play offline, on offline-mode servers, or with a legally purchased account.
- Third-party services (Modrinth, CurseForge, Polyfrost, Microsoft) have their own terms of service. You are responsible for complying with them.

---

# RemixClient (по-русски)

**Улучшенный Minecraft-лаунчер, который работает офлайн в первую очередь.** Без лицензии, без аккаунта Microsoft и без телеметрии — ввёл ник и играешь.

RemixClient — это community-форк [OneLauncher](https://github.com/Polyfrost/OneLauncher) от **Polyfrost**, переименованный и доработанный. Мы сохранили то, за что люди любили оригинал: быстрое управление версиями, работу с модами и сборками, автоматическую установку Java. И убрали то, что мешало играть: обязательный вход через Microsoft и проверки лицензии.

> **Неофициальный форк.** RemixClient **не связан** с Polyfrost, Mojang Studios, Microsoft и проектом OneLauncher, не поддерживается ими и не одобрен ими. Все торговые марки принадлежат их владельцам. См. [юридический раздел](#юридическая-информация-и-атрибуция).

## Зачем RemixClient?

- **Офлайн по умолчанию.** Создай локальный аккаунт с любым ником. RemixClient генерирует стандартный офлайн-UUID (`MD5("OfflinePlayer:<ник>")`) и запускает игру с `--userType legacy` — никаких запросов к Microsoft/Xbox/Minecraft API.
- **Без лицензионной стены.** Если игра куплена — отлично. Если хочется поиграть в одиночку, по LAN или на offline-mode сервере — аккаунт не требуется.
- **Данные остаются твоими.** Отчёты о сбоях выключены, пока мейнтейнер не укажет свой Sentry DSN. Никакой аналитики и отпечатков.
- **Привычно и быстро.** Версии, моды, сборки, Java, миры, логи и шаринг работают так же, как в знакомом лаунчере.
- **Русский из коробки.** Встроенный перевод работает без CDN и интернета.
- **Совместимость.** Старые данные OneClient, версии и аккаунты переносятся автоматически при первом запуске.

## Что уже работает

- Офлайн-аккаунты с локальным UUID и без входа в Microsoft.
- Необязательный вход через Microsoft для тех, кому нужны соцфункции.
- Менеджер версий с настройками на каждую версию (память, аргументы JVM/игры, разрешение, GPU, хуки, переменные окружения).
- Моды, ресурспаки, шейдеры и датапаки.
- Браузер и обновления Modrinth и CurseForge.
- Автоматический поиск и установка Java.
- Просмотр миров, логов и скриншотов, отчёты о сбоях и шаринг логов.
- Импорт из экспортов OneClient, Prism Launcher, MultiMC, PolyMC и Modrinth App.
- Облачная синхронизация и шаринг через сервисы Polyfrost (см. [Обновления и сервисы](#обновления-и-сервисы)).
- Встроенные английский и русский языки.

## План развития

1. **Portable-режим** — одна папка, флешка, без установщика и записей в AppData.
2. **Бэкап миров перед каждым запуском** и откат в один клик.
3. **Crash autopsy** — разбор лога, указание на сломавший мод и предложение его отключить.
4. **Офлайн-скины и плащи** — локальные файлы скинов без серверов Mojang.
5. **LAN-хостинг без Microsoft** — открыть мир в локальную сеть прямо из лаунчера.
6. **Шаринг сборок файлом** — экспорт/импорт целой версии одним файлом, без чужого облака.
7. **Проверка здоровья модов** — дубликаты, неверный загрузчик, недостающие зависимости, известные конфликты.
8. **Подписанные и воспроизводимые релизы** для Windows, Linux и macOS.
9. **Больше встроенных переводов** силами сообщества прямо в этом репозитории.

## Установка

### Windows (MSI)

Скачай последний `RemixClient_<версия>_x64_en-US.msi` из [Releases](https://github.com/linkure8888-lang/OneLauncher/releases/latest) и запусти.

Установщик регистрирует `RemixClient` как обычное приложение, создаёт ярлык в меню «Пуск» и удаляется через «Приложения и возможности». Сам бинарник внутри по-прежнему называется `oneclient_app.exe`, чтобы форк оставался совместим со сборочной системой upstream; продукт, ярлык и заголовок окна — `RemixClient`.

### Portable

Собери или скачай `oneclient_app.exe` и запускай из любой папки. Данные игры останутся в папке лаунчера, пока ты не изменишь её в *Настройки → Лаунчер → Папка лаунчера*.

### Где лежат данные

| Сборка | Папка |
|---|---|
| Release | `%APPDATA%\Remix\RemixClient` |
| Debug / разработка | `%APPDATA%\Remix\RemixClient-dev` |

При первом запуске RemixClient пытается перенести старую папку upstream (`%APPDATA%\Polyfrost\OneClient` или `OneClient-dev`) в новое место. Если перенос не удался (например, другой диск), лаунчер продолжит использовать старую папку, а не начнёт с пустой.

## Обновления и сервисы

Здесь разделены две вещи:

- **Самообновление лаунчера** идёт из GitHub Releases этого репозитория (`https://github.com/linkure8888-lang/OneLauncher/releases/latest/download/latest.json`) и проверяется публичным minisign-ключом RemixClient. Обновление никогда не заменит RemixClient обратно на upstream OneClient.
- **Контент-сервисы** — сборки, метаданные версий, CDN переводов, шаринг и Poly+ — пока используют публичные эндпоинты Polyfrost, потому что на этой экосистеме построен форк. Это их публичные сервисы, они могут измениться или быть ограничены в любой момент. Форк с ними не связан.

Для полностью независимой сборки замени константы эндпоинтов в `packages/oneclient_common/src/constants.rs` и `packages/oneclient_app/src/constants.rs` на свои.

## Сборка из исходников

### Требования

- Rust **1.97+** (stable подходит; `rustup default stable`).
- **Visual Studio Build Tools** с workload *Desktop development with C++* (линкер MSVC + Windows SDK).
- Интернет при первой сборке (крейты и git-зависимость `plusrenderer`).

### Команды (Windows PowerShell)

```powershell
git clone https://github.com/linkure8888-lang/OneLauncher.git
cd RemixClient

$env:SQLX_OFFLINE = "true"   # в репозитории лежит .sqlx

cargo test -p oneclient_auth -p oneclient_mc
cargo run -p oneclient_app
```

Если `link.exe` находится из Git, запускай сборку из *Developer PowerShell for VS* или сначала вызови `vcvars64.bat`.

### Сборка MSI

```powershell
cargo install cargo-packager --locked
$env:SQLX_OFFLINE = "true"
cargo packager -p oneclient_app --release -f wix
```

`cargo-packager` сам скачает WiX. MSI появится в `target/release/.cargo-packager/wix/`.

Для более быстрой локальной release-сборки можно переопределить тяжёлые настройки профиля:

```powershell
$env:CARGO_PROFILE_RELEASE_LTO = "thin"
$env:CARGO_PROFILE_RELEASE_CODEGEN_UNITS = "16"
```

### Подпись релизов

Сгенерируй пару minisign-ключей один раз и держи приватный ключ в секрете:

```powershell
cargo packager signer generate --path C:\keys\remixclient.key
```

Публичный ключ положи в `UPDATER_PUBKEY` в `packages/oneclient_app/src/constants.rs`, а артефакты подписывай через `CARGO_PACKAGER_SIGN_PRIVATE_KEY` (см. `cargo packager signer --help`). Никогда не коммить приватный ключ.

## Структура проекта

```
packages/
  oneclient_app/       интерфейс (Freya), онбординг, настройки, апдейтер
  oneclient_auth/      Microsoft OAuth, офлайн-аккаунты, хранилище аккаунтов
  oneclient_core/      состояние лаунчера, версии, Java, запуск, синк, шаринг
  oneclient_mc/        метаданные Minecraft, ассеты, библиотеки, аргументы, профили
  oneclient_content/   провайдеры модов, хранилище артефактов, сборки
  oneclient_cluster/   записи версий, профили настроек, файлы на диске
  oneclient_db/        слой базы данных на SQLx
  oneclient_java/      поиск и установка Java
  oneclient_net/       HTTP-транспорт, повторы, кэш, проверенные загрузки
  oneclient_polyplus/  клиент Poly+ (соцфункции и игровое время)
  oneclient_common/    пути, константы, платформа, общие типы
  oneclient_events/    шина событий (уведомления, прогресс, вопросы)
  oneclient_discord/   Discord Rich Presence
  oneclient_macro/     процедурные макросы
  polyio/              асинхронные файловые и архивные хелперы
```

## Участие в разработке

Issues и pull request'ы приветствуются. Хорошие первые задачи:

- Русский и другие переводы (`packages/oneclient_app/lang/`).
- Баг-репорты с логом из *Настройки → Лаунчер → Открыть папку лаунчера → logs*.
- Тестирование на Linux и macOS.
- Фичи из плана развития.

Перед PR запускай `cargo fmt` и `cargo clippy` и не удаляй GPL-заголовки и атрибуцию.

## Юридическая информация и атрибуция

- RemixClient распространяется по лицензии **GNU General Public License v3.0 only**. См. [LICENSE](LICENSE). Любая распространяемая сборка должна оставаться GPL-3.0 и включать соответствующий исходный код.
- RemixClient — **изменённый форк** [OneLauncher](https://github.com/Polyfrost/OneLauncher), Copyright Polyfrost и участники проекта. Оригинальные копирайты, `LICENSE` и [ATTRIBUTION.md](ATTRIBUTION.md) сохранены.
- Проект **не связан** с Polyfrost, Mojang Studios и Microsoft. *Minecraft*, *Microsoft*, *Xbox*, *OneClient* и *Polyfrost* — торговые марки их владельцев. RemixClient не претендует на них и не заявляет о поддержке с их стороны.
- Лаунчер умеет запускать Minecraft, но не выдаёт лицензию на игру. Играй офлайн, на offline-mode серверах или с легально купленным аккаунтом.
- У сторонних сервисов (Modrinth, CurseForge, Polyfrost, Microsoft) свои условия использования. Соблюдать их — твоя ответственность.
