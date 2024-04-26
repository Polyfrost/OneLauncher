# OneLauncher

Rust-based Minecraft launcher utilizing Tauri, SolidJS, and Tailwind

## Project Structure

* [`desktop`](./desktop/) - Rust-based Minecraft launcher utilizing Tauri, SolidJS, and Tailwind.
* [`core`](./core/) - The core for our Minecraft launcher (compilable without Tauri), along with Rust-based utilities for other Polyfrost projects and interactions.
* [`backend`](./backend/) - Our `actix-web` based API, accessible at <https://api.polyfrost.org>, with documentation on our [docs^2 website](https://contributing.polyfrost.org/api/).
* [`testing`](./testing/) - Testing playground for our Rust core, independant of Tauri.
* [`debugger`](./debugger/) - `proc-macro` utilities for our Rust libraries.
* [`distribution`](./distribution/) - Distribution utilties and meta files for the [`desktop`](./desktop/) app.
