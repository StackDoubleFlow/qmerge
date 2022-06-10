# Merge Applier Mod for Android

Intended to be loaded with [QuestLoader](https://github.com/sc2ad/QuestLoader).

## Compiling

Requires ndk 22 or lower
```
cargo ndk -t arm64-v8a build
```

## Creating xref traces

```
xref_gen data/libil2cpp.dbg.2019.4.28f1.so --il2cpp-metadata data/global-metadata.2019.4.28f1.dat --ignore-sections il2cpp --graph --ghidra
```