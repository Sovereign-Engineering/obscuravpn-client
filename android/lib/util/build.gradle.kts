import com.android.build.api.dsl.LibraryExtension

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlinx.serialization)
}

extensions.configure<LibraryExtension> {
    buildToolsVersion = "36.0.0"
    compileSdk = 36
    defaultConfig { minSdk = 31 }
    namespace = "net.obscura.lib.util"
}

kotlin { jvmToolchain(21) }

dependencies {
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.serialization.json)
}
