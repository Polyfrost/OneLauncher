[`JavaInfo`](./JavaInfo.java) compiled by [`LynithDev`](https://github.com/LynithDev) using
```
os.arch=amd64
java.version=1.8.0_382
java.vendor=Azul Systems, Inc.
```

> [!NOTE]
> `JavaInfo.class` is checked in rather than built, so recompile it against a Java 8
> target (`javac -source 8 -target 8 JavaInfo.java`) whenever the source changes,
> and commit both files together. [`checker.rs`](../../src/checker.rs) reads only the
> `key=value` lines it prints.
