# Getting started

You will need the tailwindcss cli. If you want to install it without going
through npm, you can do so as follows:
```
curl -sLO https://github.com/tailwindlabs/tailwindcss/releases/latest/download/tailwindcss-linux-x64
chmod +x tailwindcss-linux-x64
sudo mv tailwindcss-linux-x64 /usr/local/bin/tailwindcss
```

You will also need `trunk`, and the right wasm target
```
cargo binstall trunk
```

```
rustup target add wasm32-unknown-unknown
```

Then run `trunk serve --open` from inside the front-end you want:
`crates/web-main` for the web app, `crates/phone-main` for the phone app.
They use different ports, so both can run at once.

That is everything the web and phone front-ends need. The rest of this document
is only for working on the native phone apps.

## The native phone apps

The views for iOS and Android are Swift and Kotlin, over the Rust core in
`api-core`. [FFI.md](FFI.md) explains why, and what the boundary
guarantees.

### Android

Everything here builds on Linux. Package names below are Arch/Artix; adapt to
your distro.

```
pacman -S kotlin gradle android-tools jdk21-openjdk
```

Gradle and the Android plugin do not support JDK 26. If a newer JDK is the
default, point it back:
```
archlinux-java set java-21-openjdk
```

Linting:
```
pacman -S ktlint       # formatting
paru -S detekt-bin     # static analysis (AUR)
```

To build an APK you also need the SDK and, for the Rust cross-compile, the NDK:
```
paru -S android-sdk-cmdline-tools-latest    # AUR; gives you sdkmanager
sdkmanager "platform-tools" "platforms;android-35" "ndk;27.0.12077973"
```
`adb` and `fastboot` come from `android-tools` in the repos, so the AUR
`android-sdk-platform-tools` package is not needed and conflicts with it.

Then the Rust side:
```
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk
```

An emulator needs KVM and is fiddly without Android Studio. Testing on a
physical phone over `adb` is the easier path.

### iOS

**An iOS app cannot be built on Linux.** SwiftUI, UIKit, the simulator and
codesigning are Apple-platform only, and no toolchain works around that. A Mac
is required to build or run it.

What that means in practice is that iOS work happens in two places: the design
and the logic are worked out in `crates/phone-main`, which runs in a browser
and loops in seconds, and the SwiftUI is a transcription of a settled design
written later. The more that lives in Rust, the less is stuck behind a Mac.

Swift on Linux is therefore optional. It buys you a compiler that can
typecheck any Swift not importing SwiftUI — worth having if you are writing
Swift here, pointless otherwise:
```
paru -S swiftly-bin && swiftly init    # gives swiftc, sourcekit-lsp, swift-format
```

### Regenerating the bindings

After any change to `api-core`'s public types:
```
./scripts/bindings.sh
```
The generated Swift and Kotlin are committed, so a dirty tree afterwards means
someone changed the core without regenerating.

To check the boundary still works, which needs only `kotlinc` and `java`:
```
./scripts/kotlin-smoke.sh
```
See [FFI.md](FFI.md).
