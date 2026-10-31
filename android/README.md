# mbusio for Android

Read meters with a phone and a USB M-Bus master (USB-C/OTG adapter).

## Install

Download `mbusio-android.apk` from the [releases page](https://github.com/Wobi848/mbusio/releases),
open it on the phone and allow installing from this source. Plug in the USB master (OTG cable or
adapter) and allow mbusio to use it; with "always" Android does not ask again.

## How it works

- The app ships mbusio itself (Rust, built for `aarch64-linux-android`) as `libmbusio.so` and
  runs it as a separate process in a foreground service, so plans keep running with the screen
  off. The UI is the normal web UI in a WebView (`http://127.0.0.1:8080`).
- Android does not let programs open `/dev/ttyUSB*`. The app opens the USB serial converter
  through the USB host API ([usb-serial-for-android](https://github.com/mik3y/usb-serial-for-android):
  CH340, CP210x, FTDI, PL2303, CDC) and passes it to mbusio over a local socket. mbusio shows
  these converters as `usb://vid:pid`. Protocol: `src/link/bridge.rs`.
- A separate process instead of a library: a crash of mbusio (`panic = "abort"`) does not take
  the app down, and the service just starts it again.

## Build

Needs the Android SDK with NDK 28.2 and the Rust target `aarch64-linux-android`
(`rustup target add aarch64-linux-android`).

```sh
cd android
ANDROID_HOME=/usr/lib/android-sdk ./gradlew assembleDebug
# app/build/outputs/apk/debug/app-debug.apk
```

`assembleDebug` builds mbusio with cargo first (task `cargoBuild`). The version comes from
`Cargo.toml`. Release builds on GitHub are signed with the key from the secrets `ANDROID_KEYSTORE`
(base64) and `ANDROID_KEYSTORE_PASSWORD`; the same key is needed for every update, keep it safe.

## Limits

- arm64 phones only, Android 10 or later.
- CSV download of a chart (made in the browser) does not work in the app yet; exports from mbusio
  are saved to Downloads.
- Power: a USB master powered by the phone feeds only a few meters (GL101-U: 10 unit loads).
