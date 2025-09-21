package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class JsonFfiCommand(
    val timeoutMs: Long?,

    val getStatus: GetStatus?,
    val login: Login?,
    val logout: Logout?,
    val setApiUrl: SetApiUrl?,
    val setApiHostAlternate: SetApiHostAlternate?,
    val setSniRelay: SetSniRelay?,
    val getTrafficStats: GetTrafficStats?,
    val getExitList: GetExistList?,
    val refreshExitList: RefreshExitList?,
    val apiGetAccountInfo: ApiGetAccountInfo?,
    val setInNewAccountFlow: SetInNewAccountFlow?,
    val setPinnedExits: SetPinnedExits?,
    val rotateWgKey: RotateWgKey?,
    val setAutoConnect: SetAutoConnect?,
) {
    fun run(): Any {
        return when {
            logout != null -> {
                logout.run()
            }

            getTrafficStats != null -> {
                getTrafficStats.run()
            }

            apiGetAccountInfo != null -> {
                apiGetAccountInfo.run()
            }

            rotateWgKey != null -> {
                rotateWgKey.run()
            }

            getStatus != null -> {
                getStatus.run()
            }

            login != null -> {
                login.run()
            }

            setApiUrl != null -> {
                setApiUrl.run()
            }

            setApiHostAlternate != null -> {
                setApiHostAlternate.run()
            }

            setSniRelay != null -> {
                setSniRelay.run()
            }

            getExitList != null -> {
                getExitList.run()
            }

            refreshExitList != null -> {
                refreshExitList.run()
            }

            setInNewAccountFlow != null -> {
                setInNewAccountFlow.run()
            }

            setPinnedExits != null -> {
                setPinnedExits.run()
            }

            setAutoConnect != null -> {
                setAutoConnect.run()
            }

            else -> {
                throw NotImplementedError("JsonFfiCommand not implemented")
            }
        }
    }
}
