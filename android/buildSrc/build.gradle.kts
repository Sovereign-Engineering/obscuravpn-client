plugins {
    kotlin("jvm") version "1.9.21"

    alias(libs.plugins.kotlinx.serialization)
}

dependencies {
    implementation(gradleApi())
    implementation(gradleKotlinDsl())

    implementation(libs.kotlinx.serializationJson)
}
