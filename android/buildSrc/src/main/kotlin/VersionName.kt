import org.gradle.api.Project
import org.gradle.api.provider.Property
import org.gradle.api.provider.ValueSource
import org.gradle.api.provider.ValueSourceParameters
import org.slf4j.Logger
import org.slf4j.LoggerFactory
import java.io.File
import kotlinx.serialization.json.Json
import kotlinx.serialization.Serializable

private val logger: Logger = LoggerFactory.getLogger("version-name")

fun Project.getVersionName(projectRootDir: File): String = providers.of(VersionName::class.java) {
    it.parameters.projectRootDir.set(projectRootDir)
}.get()

abstract class VersionName : ValueSource<String, VersionName.Parameters> {
    interface Parameters : ValueSourceParameters {
        val projectRootDir: Property<File>
    }

    @Serializable
    private data class Tag(val version: String)
    private val json: Json = Json { ignoreUnknownKeys = true }

    private fun fallback(): String {
        logger.warn("building outside of nix; not intended for distribution")
        val tagString = File(parameters.projectRootDir.get().parentFile, "tag.json").readText()
        val tag = this.json.decodeFromString<Tag>(tagString)
        return "v${tag.version}.1-dev"
    }

    override fun obtain(): String {
        val version = System.getenv("OBSCURA_VERSION")
        logger.info("OBSCURA_VERSION = $version");
        return version ?: this.fallback()
    }
}
