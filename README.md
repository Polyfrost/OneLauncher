# OneLauncher

Rust-based Minecraft launcher utilizing Tauri, SolidJS, and Tailwind

## Project Structure

* [`apps/esktop`](./apps/desktop/) - Rust-based Minecraft launcher utilizing Tauri, SolidJS, and Tailwind.
* [`crates/core`](./crates/core/) - The core for our Minecraft launcher (platform agnostic), along with Rust-based utilities for other Polyfrost projects and interactions.
* [`apps/backend`](./backend/) - Our `actix-web` based API, accessible at <https://api.polyfrost.org>, with documentation on our [docs(docs)](https://contributing.polyfrost.org/api/).
* [`testing`](./crates/testing/) - Testing playground for our Rust core, independant of Tauri.
* [`distribution`](./packages/distribution/) - Distribution utilties and meta files for the [`apps/desktop`](./apps/desktop/) app.
