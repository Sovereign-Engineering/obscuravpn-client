package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class JsonFfiCommand(
    val timeoutMs: Long? = null,

    val getStatus: GetStatus? = null,
    val login: Login? = null,
    val logout: Logout? = null,
    val setApiUrl: SetApiUrl? = null,
    val setApiHostAlternate: SetApiHostAlternate? = null,
    val setSniRelay: SetSniRelay? = null,
    val getTrafficStats: GetTrafficStats? = null,
    val getExitList: GetExistList? = null,
    val refreshExitList: RefreshExitList? = null,
    val apiGetAccountInfo: ApiGetAccountInfo? = null,
    val setInNewAccountFlow: SetInNewAccountFlow? = null,
    val setPinnedExits: SetPinnedExits? = null,
    val rotateWgKey: RotateWgKey? = null,
    val setAutoConnect: SetAutoConnect? = null,
    val setFeatureFlag: SetFeatureFlag? = null,
) {
    fun run(): Any {
        return when {
            logout != null ->
                logout.run()

            getTrafficStats != null ->
                getTrafficStats.run()

            apiGetAccountInfo != null ->
                apiGetAccountInfo.run()

            rotateWgKey != null ->
                rotateWgKey.run()

            getStatus != null ->
                getStatus.run()

            login != null ->
                login.run()

            setApiUrl != null ->
                setApiUrl.run()

            setApiHostAlternate != null ->
                setApiHostAlternate.run()

            setSniRelay != null ->
                setSniRelay.run()

            getExitList != null ->
                getExitList.run()

            refreshExitList != null ->
                refreshExitList.run()

            setInNewAccountFlow != null ->
                setInNewAccountFlow.run()

            setPinnedExits != null ->
                setPinnedExits.run()

            setAutoConnect != null ->
                setAutoConnect.run()

            setFeatureFlag != null ->
                setFeatureFlag.run()

            else ->
                throw NotImplementedError("JsonFfiCommand not implemented")
        }
    }
}
