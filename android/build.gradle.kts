import com.ncorti.ktfmt.gradle.KtfmtExtension
import com.ncorti.ktfmt.gradle.TrailingCommaManagementStrategy
import io.gitlab.arturbosch.detekt.extensions.DetektExtension

plugins {
    // Only declare a plugin here if it must be loaded once rather than per-subproject
    // https://discuss.gradle.org/t/why-duplicate-plugins-in-top-level-build-scripts/49087/2
    // https://www.reddit.com/r/androiddev/comments/1errttm/comment/li1vm93/
    alias(libs.plugins.android.application) apply false
    alias(libs.plugins.android.library) apply false
    alias(libs.plugins.hilt.android) apply false
    alias(libs.plugins.kotlin.android) apply false
    alias(libs.plugins.ksp) apply false

    // These are only here for the `subprojects` block to work
    alias(libs.plugins.detekt) apply false
    alias(libs.plugins.ktfmt) apply false
}

subprojects {
    apply(plugin = rootProject.libs.plugins.detekt.get().pluginId)
    apply(plugin = rootProject.libs.plugins.ktfmt.get().pluginId)

    // https://detekt.dev/docs/gettingstarted/gradle/#kotlin-dsl-3
    extensions.configure<DetektExtension> {
        config.setFrom(rootProject.file("detekt.yml"))
        parallel = true
    }

    extensions.configure<KtfmtExtension> {
        blockIndent.set(4)
        continuationIndent.set(4)
        maxWidth.set(120)
        removeUnusedImports.set(true)
        trailingCommaManagementStrategy.set(TrailingCommaManagementStrategy.ONLY_ADD)
    }
}
