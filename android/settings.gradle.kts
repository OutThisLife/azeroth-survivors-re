// The Android app: one module, `app`, which packages the cdylib `scripts/android-build.sh` drops
// into app/src/main/jniLibs. Nothing here builds Rust; cargo-ndk does, before Gradle runs.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "azeroth-survivors"
include(":app")
