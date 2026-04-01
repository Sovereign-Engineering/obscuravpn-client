import com.android.build.api.dsl.ApplicationExtension

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.hilt.android)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlinx.serialization)
    alias(libs.plugins.ksp)
}

extensions.configure<ApplicationExtension> {
    buildToolsVersion = "36.0.0"

    namespace = "net.obscura.vpnclientapp"
    compileSdk = 36

    defaultConfig {
        applicationId = "net.obscura.vpnclientapp"
        minSdk = 31
        targetSdk = 36
        versionCode = 1
        versionName = project.getVersionName(project.rootDir)

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildFeatures {
        aidl = true
        buildConfig = true
    }

    buildTypes {
        getByName("debug") {
            applicationIdSuffix = ".debug"
            isMinifyEnabled = false
            isShrinkResources = false
            resValue("string", "app_name", "Obscura VPN (Debug)")
        }

        getByName("release") {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    flavorDimensions += listOf("billing")

    productFlavors {
        create("foss") {
            dimension = "billing"
            isDefault = true
        }

        create("play") { dimension = "billing" }
    }
}

kotlin { jvmToolchain(21) }

dependencies {
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle)
    implementation(libs.androidx.webkit)
    implementation(libs.hilt.android)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.material)
    implementation(project(":lib:util"))

    "playImplementation"(project(":lib:billing"))

    ksp(libs.hilt.android.compiler)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
}
