package net.obscura.vpnclientapp.ui

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
data class OsStatus(
    val version: String,
    val internetAvailable: Boolean,
    val osVpnStatus: OsVpnStatus,
    val srcVersion: String,
    val navigationView: NavigationView?,
    val updaterStatus: UpdaterStatus,
    val debugBundleStatus: DebugBundleStatus,
    val canSendMail: Boolean,
    val loginItemStatus: LoginItemStatus?,
    val playBilling: Boolean,
) {
    // TODO: https://linear.app/soveng/issue/OBS-2640/change-nevpnstatus-to-be-platform-agnostic
    @Serializable
    enum class OsVpnStatus {
        @SerialName("disconnected") Disconnected,
        @SerialName("connecting") Connecting,
        @SerialName("connected") Connected,
    }

    @Serializable data class LoginItemStatus(val registered: Boolean, val error: String?)

    @Serializable
    data class DebugBundleStatus(
        var inProgress: Boolean?,
        var latestPath: String?,
        var inProgressCounter: Long,
    )

    @Serializable
    enum class NavigationView {
        @SerialName("developer") Developer,
        @SerialName("connection") Connection,
        @SerialName("location") Location,
        @SerialName("account") Account,
        @SerialName("help") Help,
        @SerialName("about") About,
        @SerialName("settings") Settings;

        fun serialName() = NavigationView.serializer().descriptor.getElementName(this.ordinal)
    }

    @Serializable
    data class UpdaterStatus(
        val type: String, // TODO UpdaterStatusType
        val appcast: AppcastSummary?,
        val error: String?,
        val errorCode: Long?,
    ) {
        @Serializable
        data class AppcastSummary(
            val date: String,
            val description: String,
            val version: String,
            val minSystemVersionSdk: Boolean,
        )
    }
}
