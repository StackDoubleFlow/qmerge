cargo ndk -t arm64-v8a build
adb push ../target/aarch64-linux-android/release/libmerge_applier.so /sdcard/Android/data/com.beatgames.beatsaber/mods/libmerge_applier.so
adb logcat > test.log
