plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlinx.serialization)

    id("com.diffplug.spotless")
}

android {
    buildToolsVersion = "36.0.0"

    namespace = "net.obscura.vpnclientapp"
    compileSdk = 36

    defaultConfig {
        applicationId = "net.obscura.vpnclientapp"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
}

dependencies {

    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.appcompat)
    implementation(libs.material)
    implementation(libs.androidx.webkit)
    implementation(libs.kotlinx.serializationJson)

    testImplementation(libs.junit)
    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
}

spotless {
    java {
        target("**/*.java")

        googleJavaFormat()
        removeUnusedImports()
        trimTrailingWhitespace()
        endWithNewline()
    }

    // TODO: https://linear.app/soveng/issue/OBS-2642/move-spotless-format-check-to-nix-flake-check Don't fail build and check in CI.
    kotlin {
        target("**/*.kt")

        ktlint()
        ktfmt()
        trimTrailingWhitespace()
        endWithNewline()
    }
}

if (gradle.startParameter.taskNames.contains("nixDownloadDeps")) {
    configurations.configureEach {
        // This configuration fails to evaluate.
        if (name == "implementation") {
            exclude(module = project.name)
        }
    }

    // Some parts of the build are dynamically scheduled so aren't triggered during the dep fetch so we force the dependency.
    val lintConfig = configurations.create("nixDynamicDeps")
    dependencies {
        "nixDynamicDeps"("com.android.tools.lint:lint-gradle:31.13.0")
    }
}
