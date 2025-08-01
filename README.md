<div align="center">

<img src=".github/media/RepoBanner.png" alt="Repository Banner" />

# OneLauncher  |  OneClient
The monorepo containing the code for OneLauncher, OneClient, and their core backend.

</div>

## Technologies Used
- [**Tauri**](https://tauri.app/)
- [**Sea ORM**](https://www.sea-ql.org/SeaORM/)
- [**SQLite**](https://www.sqlite.org/)
- [**React**](https://react.dev/)
- [**Tailwind CSS**](https://tailwindcss.com/)
- [**Tanstack Query**](https://tanstack.com/query/latest)
- [**Tanstack Router**](https://tanstack.com/router/latest)

## Contributing
We welcome contributions! Please read our [contributing guidelines](CONTRIBUTING.md) before getting started.

### Requirements

If you encounter any issues, ensure that you are using the following versions (or later) of Rust, Node and Pnpm:

- `rustc` version: **nightly-2025-07-23**
- `node` version: **22.15**
- `pnpm` version: **10.13**

### Project Structure
- **`packages/`** - Shared libraries and utilities.
  - [**`core/`**](./packages/core/) - Launcher Core. This is the library that contains the entire launcher logic.
  - [**`entity/`**](./packages/entity/) - Contains entity definitions used in the launcher's database.
  - [**`macro/`**](./packages/macro/) - Contains macro definitions to simplify some code.
  - [**`scripts/`**](./packages/scripts/) - General scripts for CI/CD or development environments.
  - [**`web_common/`**](./packages/web_common/) - Contains common web code used by both OneLauncher and OneClient.
- **`apps/`**
  - [**`onelauncher/`**](./apps/onelauncher/)
    - **`desktop/`** - Tauri desktop application.
	- **`frontend/`** - React SPA frontend.
	- **`distribution/`** - Files used for release builds.
  - [**`oneclient/`**](./apps/oneclient/)
    - **`desktop/`** - Tauri desktop application.
	- **`frontend/`** - React SPA frontend.
	- **`distribution/`** - Files used for release builds.
