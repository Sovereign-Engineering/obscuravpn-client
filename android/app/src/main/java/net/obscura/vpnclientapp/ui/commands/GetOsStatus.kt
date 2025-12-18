package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.helpers.currentApp

@Serializable
data class GetOsStatus(
    val knownVersion: String? = null,
) {
  companion object {
    var initialized = false
  }

  @Serializable
  data class Result(
      val version: String,
      val internetAvailable: Boolean,
      val osVpnStatus: NEVPNStatus?,
      val srcVersion: String,
      val updaterStatus: UpdaterStatus,
      val debugBundleStatus: DebugBundleStatus,
      val canSendMail: Boolean,
      val loginItemStatus: LoginItemStatus?,
  ) {
    // TODO https://linear.app/soveng/issue/OBS-2640/change-nevpnstatus-to-be-platform-agnostic Enum
    // should be platform agnostic.
    @Serializable
    enum class NEVPNStatus {
      @SerialName("invalid") Invalid,
      @SerialName("disconnected") Disconnected,
      @SerialName("connecting") Connecting,
      @SerialName("connected") Connected,
      @SerialName("reasserting") Reasserting,
      @SerialName("disconnecting") Disconnecting,
    }

    @Serializable
    data class LoginItemStatus(
        val registered: Boolean,
        val error: String?,
    )

    @Serializable
    data class DebugBundleStatus(
        val inProgress: Boolean?,
        val latestPath: String?,
        val inProgressCounter: Long,
    )

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

  fun run(context: Context): CompletableFuture<Result> =
      context.currentApp().osStatus.getStatus(knownVersion)
}
