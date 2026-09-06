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

Then run `trunk serve --open` from `crates/web-main`, which is the web app.

It reads its data from `server-main`, which is what holds the Plaid secret —
see [PLAID.md](PLAID.md) for why that cannot live in the app. So the web app is
two commands:

```
cargo run -p server-main                  # :8082
cd crates/web-main && trunk serve         # :8080
```

**The server needs `.env` filled in, and it needs Postgres.** Every variable is
required — Plaid's credentials, the environment, the collection channel and
`DATABASE_URL` — and it exits saying which one is missing rather than starting
and serving something plausible.

Postgres is not optional because the Plaid access tokens live in it, one row
per bank:

```
cp .env.example .env
./scripts/db-init.sh                      # Postgres in a container
cargo run -p server-main --example link_sandbox   # a bank to look at
cargo run -p server-main                  # applies migrations, then serves
```

Without that middle line the server starts and serves an empty ledger, which is
correct: no bank has been linked. [PLAID.md](PLAID.md) has the rest.

`./scripts/dev.sh` brings both up as they are deployed and follows their logs.
Give it a window and leave it there: it starts the containers detached, so
`./scripts/dev.sh restart server` from any other shell lands its output back in
that window rather than in whichever terminal ran it.
[DATABASE.md](DATABASE.md) has the rest, including the one rule about
migrations.

That is everything the web front-end needs. The rest of this document is only
for working on the native phone apps.

## The native phone apps

The views for iOS and Android are Swift and Kotlin, over the Rust core in
`phone-state`. [FFI.md](FFI.md) explains why, and what the boundary
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

What that means in practice is that as little of the app as possible waits on a
Mac. The logic is `phone-state`, which is plain Rust tested with `eval!` and
needs no phone at all; the design is settled in
`sketches/design-language.html`, which is a page. The SwiftUI is a
transcription of the two. The more that lives in Rust, the less is stuck behind
a Mac.

Swift on Linux is therefore optional. It buys you a compiler that can
typecheck any Swift not importing SwiftUI — worth having if you are writing
Swift here, pointless otherwise:
```
paru -S swiftly-bin && swiftly init    # gives swiftc, sourcekit-lsp, swift-format
```

#### On a Mac

Xcode, but very nearly only for its command line tools — there is no
`.xcodeproj` in the tree. `swiftc`, `xcodebuild`, `xcrun` and `simctl` are the
whole toolchain, so the work happens in whatever editor you already use. The
one exception is the development certificate a real phone needs, which only
Xcode can mint; [On a real phone](#on-a-real-phone) has it, and it is once per
machine.

Xcode 26 ships without a simulator runtime, and it is a large download rather
than a quick one:
```
xcodebuild -downloadPlatform iOS
```

The Rust core cross-compiles to the phone, which needs the targets. The
simulator's architecture is the Mac's, so an Intel Mac wants `x86_64-apple-ios`
and an Apple Silicon one wants `aarch64-apple-ios-sim`; `aarch64-apple-ios` is
the device either way:
```
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
```

#### Running the app

Two commands, like the web front-end: the server that holds the Plaid secret,
and the app.

```
cargo run -p server-main       # :8082
./scripts/ios-run.sh           # builds, installs, launches
```

The simulator reaches the server over the Mac's own loopback, so both of these
are the same `127.0.0.1:8082` the web app uses — the script writes that address
into the bundle it builds, over the hosted server the plist names.
`MERIDIAN_SERVER=https://api.meridiancash.io` points the simulator at the box
instead. A different simulator is an argument — any name from `xcrun simctl
list devices`:

```
./scripts/ios-run.sh "iPhone 16e"
```

The Simulator is an ordinary macOS window. Put it on whichever screen you want
it on and leave it there: a re-run installs into the device that is already
booted rather than starting a new one, so the window neither moves nor
reopens — the app just becomes the new build. **Window → Stay On Top** keeps it
above whatever you are editing in.

Nothing above opens Xcode, and there is no project file to open. `swiftc`
compiles, `simctl` installs and launches, and `compile_commands.json` is
rewritten on every run from the flags that built the app, so sourcekit-lsp sees
exactly what the compiler saw.

#### On a real phone

The simulator asks nothing of Apple. A real phone asks for three things, and
only the last of them repeats.

**Once on the phone.** Settings → Privacy & Security → Developer Mode, on. The
phone restarts. Plug it in, unlock it, and trust the Mac when it asks.

**Once on the Mac.** A development certificate, which is the one part of this
that Xcode has to do. Apple issues certificates to a signed-in Apple ID and
there is no `xcrun` that asks for one, so:

1. Xcode → Settings → Accounts → **+** → Apple ID, and sign in
2. Select the account, **Manage Certificates…**, then **+** → **Apple
   Development**

Step 2 matters. Signing in alone often mints nothing — Xcode creates the
certificate the first time it signs something, and since there is no project
here it never gets around to it. Doing it by hand is what makes it exist.

A free Apple ID is enough. It issues the same "Apple Development" certificate a
paid account does, and everything below treats the two the same.

That is the last of Xcode. Close it.

**Weekly.** The provisioning profile, which says which phones the build may run
on. A free account's lasts seven days:

```
./scripts/ios-provision.sh
```

Apple issues a profile to a build rather than to a request, so that script
writes a throwaway Xcode project under `target/`, builds it against the phone
to collect the profile, and throws the build away. The real app never involves
a project. Run it again whenever `ios-device.sh` says the profile has expired,
and whenever a new phone needs adding — a profile names the phones it was cut
for, and one that was not plugged in when it was issued will not run the app.

Then the app itself, two commands like the simulator's:

```
cargo run -p server-main       # :8082
./scripts/ios-device.sh        # builds, signs, installs, launches
```

A phone cannot reach the Mac's loopback, so `ios-device.sh` looks up the Mac's
address on the LAN and writes it into the bundle it builds. Both machines have
to be on the same network, and a network change means building again.
`MERIDIAN_SERVER=http://host:8082` names the address yourself when the server
is somewhere else, and `MERIDIAN_SERVER=https://api.meridiancash.io` points the
phone at the hosted server. A second phone is an argument, as the simulator's
is — any identifier from `xcrun devicectl list devices`.

The very first launch is refused, with the phone saying the developer is not
trusted. That is Apple asking you to vouch for your own certificate, and it is
once per certificate rather than per build: Settings → General → VPN & Device
Management → your Apple ID → Trust. Then run `ios-device.sh` again.

**When something fails.** Three failures look alike from the phone, which
reports every one of them as "Unable to Install" or a launch that is refused:

- *the profile expired* — `ios-device.sh` checks the date before it builds and
  says so; run `ios-provision.sh`
- *the certificate cannot be chained* — `security find-identity -v -p
  codesigning` reports "0 valid identities" even though the certificate is
  right there. The intermediate that signed it is missing from the keychain.
  Apple keeps the current ones at
  [apple.com/certificateauthority](https://www.apple.com/certificateauthority/);
  match your certificate's issuer (`security find-certificate -c "Apple
  Development" -p | openssl x509 -noout -issuer`) and `security import` it
- *the phone was never trusted* — the Settings path above

**A phone the Mac cannot see at all.** This one looks different: nothing
reaches the phone. `xcrun devicectl list devices` reports it as `unavailable`
and `ios-device.sh` dies at the install with "CoreDeviceService was unable to
locate a device matching the requested device identifier". The phone can be
enumerating over USB perfectly — right serial, 480 Mb/s — and still look like
this, because what is stale is the pairing record, not the cable.

The tell is in the daemon log, where `log` is usually shadowed by a shell
function, so spell it out:

```
/usr/bin/log show --last 5m --predicate 'process == "usbmuxd"' --style compact
# E usbmuxd Could not start session with device: kAMDInvalidHostIDError
```

The Mac offers a host identity the phone no longer recognises, lockdownd
refuses the session, and the device is dropped as fast as it attaches — which
is why `devicectl` keeps answering from a cached record whose
`lastConnectionDate` never moves. **Forget the trusted computers and trust the
Mac again: Settings → Developer → Clear Trusted Computers, then replug and tap
Trust with the phone unlocked.** The next attach logs "Successfully paired".

Reach for this before rebooting. A reboot clears a different fault — IOKit
refusing usbmuxd a plug-in interface, which logs `Ignored device,
IOCreatePlugInInterfaceForService failed: kIOReturnNoResources` — and does
nothing at all for a stale host ID.

### Regenerating the bindings

After any change to `phone-state`'s public types:
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
