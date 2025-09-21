package net.obscura.vpnclientapp.ui.commands

import androidx.core.os.BuildCompat
import kotlinx.serialization.Serializable

@Serializable
data class GetOsStatus(
    val knownVersion: String?
) {

    @Serializable
    data class Result(
        val version: String,
        val internetAvailable: Boolean,
        val osVpnStatus: String?, // TODO NEVPNStatus
        val srcVersion: String,
        val strictLeakPrevention: Boolean,
        val updaterStatus: UpdaterStatus,
        val debugBundleStatus: DebugBundleStatus,
        val canSendMail: Boolean,
        val loginItemStatus: LoginItemStatus?,
    ) {
        @Serializable
        data class LoginItemStatus(
            val registered: Boolean,
            val error: String?
        )

        @Serializable
        data class DebugBundleStatus(
            val inProgress: Boolean?,
            val latestPath: String?,
            val inProgressCounter: Long
        )

        @Serializable
        data class UpdaterStatus(
            val type: String, // TODO UpdaterStatusType
            val appcast: AppcastSummary?,
            val error: String?,
            val errorCode: Long?
        ) {
            @Serializable
            data class AppcastSummary(
                val date: String,
                val description: String,
                val version: String,
                val minSystemVersionSdk: Boolean
            )
        }
    }

    fun run(): Result {
        return Result(
            version = android.os.Build.VERSION.BASE_OS,
            internetAvailable = true,
            osVpnStatus = null,
            srcVersion = "TODO",
            strictLeakPrevention = true,
            updaterStatus = Result.UpdaterStatus(
                type = "uninitiated",
                appcast = null,
                error = null,
                errorCode = null
            ),
            debugBundleStatus = Result.DebugBundleStatus(
                inProgress = false,
                latestPath = null,
                inProgressCounter = 0
            ),
            canSendMail = false,
            loginItemStatus = null
        )
    }
}
