cargo ndk -t arm64-v8a build --release

adb push ../target/aarch64-linux-android/release/libmerge_applier.so /sdcard/Android/data/com.beatgames.beatsaber/files/mods/libmerge_applier.so
adb shell am force-stop com.beatgames.beatsaber
adb shell am start com.beatgames.beatsaber/com.unity3d.player.UnityPlayerActivity

adb logcat -c && adb logcat > test.log