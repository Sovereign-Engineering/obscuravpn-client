plugins {
    alias(libs.plugins.kotlin.jvm)
    alias(libs.plugins.kotlinx.serialization)
}

dependencies {
    implementation(gradleKotlinDsl())
    implementation(libs.kotlinx.serialization.json)
}
