# UniFFI / JNA rules
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn com.sun.jna.**

# Keep all UniFFI-generated classes under uniffi.oubli
-keep class uniffi.oubli.** { *; }

# Keep PlatformStorageCallback implementations
-keep class com.oubli.wallet.platform.KeystoreStorage { *; }

# Standard Android rules
-keepattributes *Annotation*
-keepattributes SourceFile,LineNumberTable
