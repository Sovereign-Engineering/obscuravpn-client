import com.android.build.api.dsl.LibraryExtension

plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
}

extensions.configure<LibraryExtension> {
    buildToolsVersion = "36.0.0"
    compileSdk = 36
    defaultConfig { minSdk = 31 }
    namespace = "net.obscura.lib.billing"
}

kotlin { jvmToolchain(21) }

dependencies {
    implementation(libs.android.billingclient)
    implementation(libs.kotlin.stdlib)
    implementation(libs.kotlinx.coroutines.playServices)
    implementation(libs.playServices.base)
    // This is a dep of `billingclient`, but we specify it manually to override
    // an outdated dependency version:
    // https://github.com/mullvad/mullvadvpn-app/pull/9887
    implementation(libs.playServices.location)
    implementation(project(":lib:util"))
}
