#!/bin/sh
# Compiles the generated Kotlin bindings and runs them against the real Rust
# library, proving the FFI boundary end to end.
#
# There is no Gradle project yet — no Android app to attach one to — so this
# does it by hand with kotlinc. JNA is fetched once into target/.
#
# Needs: kotlinc, java, and a network fetch the first time.

set -e

cd "$(dirname "$0")/.."

JNA_VERSION=5.14.0
OUT=target/kotlin-smoke
JNA=$OUT/jna-$JNA_VERSION.jar
STDLIB=/usr/share/kotlin/lib/kotlin-stdlib.jar

mkdir -p $OUT

[ -f "$JNA" ] || curl -sL -o "$JNA" \
    "https://repo1.maven.org/maven2/net/java/dev/jna/jna/$JNA_VERSION/jna-$JNA_VERSION.jar"

cargo build -p phone-state
cp target/debug/libphone_state.so $OUT/

kotlinc -classpath "$JNA" \
    kotlin/src/main/kotlin/uniffi/phone_state/phone_state.kt \
    -d $OUT/bindings.jar

kotlinc -classpath "$JNA:$OUT/bindings.jar" \
    kotlin/src/test/kotlin/Smoke.kt \
    -d $OUT/smoke.jar

java -Djna.library.path=$OUT \
    -classpath "$JNA:$OUT/bindings.jar:$OUT/smoke.jar:$STDLIB" \
    SmokeKt
