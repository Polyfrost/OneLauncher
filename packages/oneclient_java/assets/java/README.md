[`JavaInfo`](./JavaInfo.java) compiled by [`LynithDev`](https://github.com/LynithDev) using
```
os.arch=amd64
java.version=1.8.0_382
java.vendor=Azul Systems, Inc.
```

> [!NOTE]
> The checked-in `JavaInfo.class` predates the `java.home`, `java.awt` and `java.jdk`
> lines and does not print them yet. Recompiling it against a Java 8 target
> (`javac --release 8 JavaInfo.java`) is what turns the probe-side answers on; until
> then [`checker.rs`](../../src/checker.rs) falls back to inspecting the installation
> on disk, which reaches the same verdict for every image seen in the wild.
