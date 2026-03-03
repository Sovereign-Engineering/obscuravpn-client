import com.android.build.api.dsl.ApplicationExtension
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlinx.serialization)
    alias(libs.plugins.spotless)
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
            isMinifyEnabled = false
            isShrinkResources = false
        }

        getByName("release") {
            isMinifyEnabled = true
            isShrinkResources = true
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
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_11
    }
}

dependencies {
    implementation(libs.androidx.appcompat)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.webkit)
    implementation(libs.kotlinx.serialization.json)
    implementation(libs.material)

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
    @Suppress("UNUSED_VARIABLE")
    val lintConfig = configurations.create("nixDynamicDeps")
    dependencies {
        "nixDynamicDeps"("com.android.tools.lint:lint-gradle:31.13.0")
    }
}
