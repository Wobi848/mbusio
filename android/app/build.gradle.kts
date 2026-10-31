plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "io.github.wobi848.mbusio"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "io.github.wobi848.mbusio"
        // Android 10: Speichern in „Downloads“ ohne Speicherberechtigung
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "0.5.0"
        ndk { abiFilters += "arm64-v8a" }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    // mbusio läuft als eigener Prozess aus nativeLibraryDir: dafür muss die Datei ausgepackt sein
    packaging { jniLibs { useLegacyPackaging = true } }
}

kotlin { jvmToolchain(17) }

dependencies {
    implementation("com.github.mik3y:usb-serial-for-android:3.9.0")
}

// mbusio (Rust) für Android bauen und als libmbusio.so beilegen: nur so darf die App es starten
val cargoBuild by tasks.registering(Exec::class) {
    val repo = rootDir.parentFile
    val bin = android.ndkDirectory.resolve("toolchains/llvm/prebuilt/linux-x86_64/bin")
    val clang = bin.resolve("aarch64-linux-android29-clang").path
    workingDir = repo
    environment("CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER", clang)
    environment("CC_aarch64_linux_android", clang)
    environment("AR_aarch64_linux_android", bin.resolve("llvm-ar").path)
    commandLine("cargo", "build", "--release", "--target", "aarch64-linux-android")
    doLast {
        copy {
            from(repo.resolve("target/aarch64-linux-android/release/mbusio"))
            into(projectDir.resolve("src/main/jniLibs/arm64-v8a"))
            rename { "libmbusio.so" }
        }
    }
}
tasks.named("preBuild") { dependsOn(cargoBuild) }
