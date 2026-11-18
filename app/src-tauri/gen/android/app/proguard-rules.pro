# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# SAF JNI surface (#148) — every @JvmStatic method below is called ONLY from
# Rust via JNI (src-tauri/src/saf_android.rs), so R8 sees zero Java references
# and strips them in release builds. Symptom: "Java exception was raised during
# method invocation" on any SAF invoke. Keep the class + its companion (the
# @JvmStatic methods live on MainActivity$Companion).
# NOTE: keep in sync with `identifier` in tauri.conf.json and the package of
# the generated MainActivity. Only one package name belongs in this file.
-keep class app.zeromd.MainActivity { *; }
-keep class app.zeromd.MainActivity$Companion { *; }
