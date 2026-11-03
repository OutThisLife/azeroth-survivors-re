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
        versionCode = 3
        versionName = "0.2.2"
        // Only the arm64 library is built (scripts/android-build.sh); no 32-bit or x86 slices.
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }

    buildTypes {
        release {
            // Nothing to shrink: one activity class, the game is the .so.
            isMinifyEnabled = false
            // Unsigned until the operator signs with apksigner (docs/ANDROID.md, P6).
        }
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
