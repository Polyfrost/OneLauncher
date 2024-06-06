# OneLauncher

Rust-based Minecraft launcher utilizing Tauri, SolidJS, and Tailwind

## Project Structure

* [`desktop`](./desktop/) - Rust-based Minecraft launcher utilizing Tauri, SolidJS, and Tailwind.
* [`core`](./core/) - The core for our Minecraft launcher (platform agnostic), along with Rust-based utilities for other Polyfrost projects and interactions.
* [`backend`](./backend/) - Our `actix-web` based API, accessible at <https://api.polyfrost.org>, with documentation on our [docs(docs)](https://contributing.polyfrost.org/api/).
* [`testing`](./testing/) - Testing playground for our Rust core, independant of Tauri.
* [`debugger`](./debugger/) - Procedural macro utilities for our Rust libraries.
* [`distribution`](./distribution/) - Distribution utilties and meta files for the [`desktop`](./desktop/) app.
