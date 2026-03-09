plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlinx.serialization)
}

dependencies {
    implementation(gradleKotlinDsl())
    implementation(libs.kotlinx.serialization.json)

    // Workaround to ensure this dependency makes it into the Nix MiTM cache
    // (Required by the task `extractDebugAnnotations`/`extractReleaseAnnotations`)
    @Suppress("UNUSED_VARIABLE")
    val lintConfig = configurations.create("nixDynamicDeps")
    "nixDynamicDeps"("com.android.tools.lint:lint-gradle:31.13.0") // Version must match what's expected by AGP
}
