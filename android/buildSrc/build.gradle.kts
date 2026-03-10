import com.ncorti.ktfmt.gradle.KtfmtExtension
import com.ncorti.ktfmt.gradle.TrailingCommaManagementStrategy
import io.gitlab.arturbosch.detekt.extensions.DetektExtension
import org.gradle.kotlin.dsl.configure

plugins {
    alias(libs.plugins.detekt)
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlinx.serialization)
    alias(libs.plugins.ktfmt)
}

dependencies {
    implementation(gradleKotlinDsl())
    implementation(libs.kotlinx.serialization.json)

    // Workaround to ensure this dependency makes it into the Nix MiTM cache
    // (Required by the task `extractDebugAnnotations`/`extractReleaseAnnotations`)
    @Suppress("UNUSED_VARIABLE") val lintConfig = configurations.create("nixDynamicDeps")
    "nixDynamicDeps"(
        "com.android.tools.lint:lint-gradle:31.13.0" // Version must match what's expected by AGP
    )
}

extensions.configure<DetektExtension> {
    config.setFrom(rootProject.file("detekt.yml"))
    parallel = true
}

extensions.configure<KtfmtExtension> {
    kotlinLangStyle()
    maxWidth.set(120)
    removeUnusedImports.set(true)
    trailingCommaManagementStrategy.set(TrailingCommaManagementStrategy.ONLY_ADD)
}
