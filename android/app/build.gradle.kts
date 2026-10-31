plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

// Version wie mbusio selbst (Cargo.toml), nicht ein zweites Mal pflegen
val cargoVersion = Regex("""(?m)^version = "(\d+)\.(\d+)\.(\d+)"""")
    .find(rootDir.parentFile.resolve("Cargo.toml").readText())!!.groupValues
// Release-Signatur nur auf GitHub (Secrets, siehe .github/workflows/build.yml); lokal unsigniert
val keystore = System.getenv("ANDROID_KEYSTORE_FILE")

android {
    namespace = "io.github.wobi848.mbusio"
    compileSdk = 36
    ndkVersion = "28.2.13676358"

    defaultConfig {
        applicationId = "io.github.wobi848.mbusio"
        // Android 10: Speichern in „Downloads“ ohne Speicherberechtigung
        minSdk = 29
        targetSdk = 36
        versionCode = cargoVersion[1].toInt() * 10000 + cargoVersion[2].toInt() * 100 + cargoVersion[3].toInt()
        versionName = cargoVersion.drop(1).joinToString(".")
        ndk { abiFilters += "arm64-v8a" }
    }

    signingConfigs {
        if (keystore != null) {
            create("release") {
                storeFile = file(keystore)
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = "mbusio"
                keyPassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.findByName("release")
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
