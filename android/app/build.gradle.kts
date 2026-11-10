import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "io.github.songoku119.azerothsurvivors"
    // 34, not 35: 35 enforces edge-to-edge and needs inset handling on the HUD (docs/ANDROID.md, P6).
    compileSdk = 34

    defaultConfig {
        applicationId = "io.github.songoku119.azerothsurvivors"
        // 29: scoped storage and the Vulkan 1.1 floor the manifest's uses-feature asks for.
        minSdk = 29
        targetSdk = 34
        versionCode = 8
        versionName = "0.2.7"
        // Only the arm64 library is built (scripts/android-build.sh); no 32-bit or x86 slices.
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    // The release key, from the environment the release workflow sets out of the repository's
    // secrets (scripts/android-keystore.sh makes the key and names the secrets). A build without
    // them, a local `make android`, signs release with the debug key instead and says so: it
    // still installs, but not over a release-signed one.
    // The workflow sets the variable to "" when it has no key: not a file, not an error.
    val releaseKeystore = System.getenv("ANDROID_KEYSTORE_FILE")?.takeUnless { it.isEmpty() }?.let { file(it) }?.takeIf { it.isFile }
    val releaseSigned = releaseKeystore != null &&
        !System.getenv("ANDROID_KEYSTORE_PASSWORD").isNullOrEmpty() &&
        !System.getenv("ANDROID_KEY_ALIAS").isNullOrEmpty()
    signingConfigs {
        create("release") {
            if (releaseSigned) {
                storeFile = releaseKeystore
                storePassword = System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = System.getenv("ANDROID_KEY_ALIAS")
                // An absent Actions secret arrives as "", not null.
                keyPassword = System.getenv("ANDROID_KEY_PASSWORD")?.takeUnless { it.isEmpty() }
                    ?: System.getenv("ANDROID_KEYSTORE_PASSWORD")
                enableV2Signing = true
                enableV3Signing = true
            }
        }
    }

    buildTypes {
        release {
            // Nothing to shrink: one activity class, the game is the .so.
            isMinifyEnabled = false
            signingConfig = if (releaseSigned) signingConfigs.getByName("release") else signingConfigs.getByName("debug")
            // Debuggable on purpose for now: `adb shell run-as` reaches env.toml and the status
            // file only in a debuggable app, and this is a sideloaded build for its own players.
            // Flip to false for a build meant for strangers.
            isDebuggable = true
        }
    }
    if (!releaseSigned) {
        println("android: no ANDROID_KEYSTORE_FILE/PASSWORD/KEY_ALIAS in the environment; release is debug-signed (scripts/android-keystore.sh)")
    }

    // The .so arrives prebuilt in src/main/jniLibs from cargo-ndk; Gradle only packages it.
    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    packaging {
        jniLibs {
            // Stored uncompressed and page-aligned so the loader maps the .so from the APK;
            // the 16 KB alignment check in android-build.sh assumes this.
            useLegacyPackaging = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(JvmTarget.JVM_17)
    }
}

dependencies {
    // GameActivity. The android-activity 0.6.1 README names 4.4.0, compatible back to 4.0.0;
    // a line past what android-activity's native glue was built for crashes at startup, so
    // this moves only with the Rust crate.
    implementation("androidx.games:games-activity:4.4.0")
    // GameActivity extends AppCompatActivity: the theme in res/values/styles.xml is an
    // AppCompat one for that reason.
    implementation("androidx.appcompat:appcompat:1.7.0")
}
