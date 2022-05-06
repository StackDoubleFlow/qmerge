# qmerge

C# Unity IL2CPP modding framework

## Features

Here is what qmerge can currently accomplish:

- No JIT or Mono Runtime
- Low performance overhead

## Goals

These will be moved over to the features list one day:

- Method patching attributes (Prefix and Postfix)
- Standard build system with build scripts making setup extremely easy for new users and very flexible for the advanced
- Compatibility with mods using [beatsaber-hook](https://github.com/sc2ad/beatsaber-hook)
- Semantic versioning and API Stability 

## Platform support

Operating System | Architecture | Support
--- | --- | ---
Android | ARMv8 (A64) | In Progress
Android | ARMv7 | Unsupported
Windows | x86 | Unsupported
Windows | x86_64 | Unsupported

## Version support

Unity version | IL2CPP version | Support
--- | --- | ---
4.6.1+ | First release | Unsupported
5.2.x | 15 | Unsupported
5.3.0-5.3.1 | 16 | Unsupported
5.3.2 | 19 | Unsupported
5.3.3-5.3.4 | 20 | Unsupported
5.3.5-5.4.6 | 21 | Unsupported
5.5.0-5.5.6 | 22 | Unsupported
5.6.0-5.6.7 | 23 | Unsupported
2017.1.0-2018.2.21 | 24 | Unsupported
2018.3.0-2018.4.x | 24.1 | Unsupported
2019.1.0-2019.3.6 | 24.2 | Unsupported
2019.3.7-2019.4.14 | 24.3 | Unsupported
2019.4.15-2019.4.20 | 24.4 | In Progress
2019.4.21-2019.4.x | 24.5 | Unsupported
2020.1.0-2020.1.10 | 24.3 | Unsupported
2020.1.11-2020.1.17 | 24.4 | Unsupported
2020.2.0-2020.2.3 | 27 | Unsupported
2020.2.4-2020.3.x | 27.1 | Unsupported
2021.1.0-2021.1.x | 27.2 | Unsupported

## Acknowledgements and Other

These are some of people and their repositories that have been a tremendous help in the development of qmerge:

- [djkaty](https://github.com/djkaty) - [Il2CppInspector](https://github.com/djkaty/Il2CppInspector)
- [zoller27osu](https://github.com/zoller27osu), [sc2ad](https://github.com/sc2ad) and [jakibaki](https://github.com/jakibaki) - [beatsaber-hook](https://github.com/sc2ad/beatsaber-hook)
- [sc2ad](https://github.com/sc2ad) and [nike4613](https://github.com/nike4613) - [QuestLoader](https://github.com/sc2ad/QuestLoader)
- [StackDoubleFlow](https://github.com/StackDoubleFlow) and [raftario](https://github.com/raftario) - [quest-hook-rs](https://github.com/StackDoubleFlow/quest-hook-rs)
- [raftario](https://github.com/raftario) - [paranoid-android](https://github.com/raftario/paranoid-android)
- [StackDoubleFlow](https://github.com/StackDoubleFlow) - [brocolib](https://github.com/StackDoubleFlow/brocolib), [xref_gen](https://github.com/StackDoubleFlow/xref_gen), [Merge](https://github.com/StackDoubleFlow/Merge)
