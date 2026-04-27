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
}

extensions.configure<DetektExtension> {
    config.setFrom(rootProject.file("../detekt.yml"))
    parallel = true
}

extensions.configure<KtfmtExtension> {
    kotlinLangStyle()
    maxWidth.set(120)
    removeUnusedImports.set(true)
    trailingCommaManagementStrategy.set(TrailingCommaManagementStrategy.ONLY_ADD)
}
