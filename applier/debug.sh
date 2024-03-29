#!/bin/bash
# set -x

cargo ndk -t arm64-v8a build --release

adb push ../target/aarch64-linux-android/release/libmerge_applier.so /sdcard/Android/data/com.beatgames.beatsaber/files/mods/libmerge_applier.so

# Run lldb-server in the background
# adb shell "cat /data/local/tmp/lldb-server | run-as com.beatgames.beatsaber sh -c 'cat > /data/data/com.beatgames.beatsaber/lldb/bin/lldb-server && chmod 700 /data/data/com.beatgames.beatsaber/lldb/bin/lldb-server'"
# adb shell run-as com.beatgames.beatsaber ./lldb/bin/lldb-server platform --listen "*:42069" --server &

# Forward port to connect to lldb-server
adb forward tcp:42069 tcp:42069

# Start game
# adb shell am set-debug-app -w com.beatgames.beatsaber
adb shell am start -S -W com.beatgames.beatsaber/com.unity3d.player.UnityPlayerActivity

# Wait for game process to start and mods to load
# sleep 8

# Get pid of game process and format it into the debugger url
debugPid=$(adb shell pidof com.beatgames.beatsaber)
# This url contains the debugger configuration that gets used when starting the debugger
debugUrl="vscode://vadimcn.vscode-lldb/launch/config?{
    request: 'attach', 
    pid: $debugPid, 
    program: '', 
    initCommands: [
        'platform select remote-android',
        'settings set target.inherit-env false',
        'platform connect connect://1WMHH816UT0372:42069'
    ],
    postRunCommands: [
        'pro hand -p true -s false SIGPWR',
        'pro hand -p true -s false SIGXCPU',
        'pro hand -p true -s false SIG33',
    ]
}"

# Run CodeLLDB extension debugger
code --open-url "$debugUrl"
