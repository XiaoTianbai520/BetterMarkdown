# BetterMarkdown Android Build & Test Scripts

Scripts for building, testing, and packaging BetterMarkdown for Android (`main` branch).

## Android Release Build

Build signed release APKs and AAB:

```bash
./scripts/build-android.sh
```

Fast iteration build without signing or ProGuard:

```bash
./scripts/build-android.sh --debug
```

### Outputs

Located under `app/src-tauri/gen/android/app/build/outputs/`:
- `apk/{arm64-v8a,armeabi-v7a,x86_64,universal}/release/app-*-release.apk`
- `bundle/release/app-release.aab` (Google Play format)

### Environment Variables

Configure the following in `.env.local` or environment:
- `ANDROID_HOME`: Path to Android SDK
- `ANDROID_NDK_HOME`: Path to Android NDK (NDK 26+)
- `ANDROID_KEYSTORE_PATH`: Path to release keystore (`.jks`)
- `ANDROID_KEYSTORE_PASS`: Keystore password
- `ANDROID_KEY_ALIAS`: Key alias
- `ANDROID_KEY_PASS`: Key password

## Android Regression Test Suite

Automated end-to-end regression tests driven via ADB:

```bash
# Run full Android regression suite against connected device / emulator
./scripts/regress/android/run.sh

# Target specific tests
./scripts/regress/android/run.sh A10 search
```

See [scripts/regress/android/README.md](regress/android/README.md) for full details.

## Checksums

Verify or generate SHA256 checksums:

```bash
./scripts/checksums.sh <tag>
```
