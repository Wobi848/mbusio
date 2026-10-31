pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
        // usb-serial-for-android gibt es nur dort
        maven("https://jitpack.io")
    }
}

rootProject.name = "mbusio"
include(":app")
