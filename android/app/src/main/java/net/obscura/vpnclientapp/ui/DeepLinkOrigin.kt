package net.obscura.vpnclientapp.ui

/**
 * Provenance of an `obscuravpn://` deep link, derived by [net.obscura.vpnclientapp.activities.MainActivity]
 * before dispatch.
 *
 * Externally supplied URIs (other apps, browsers via the BROWSABLE filter, or adb) carry no
 * trustworthy provenance — Intent extras and [android.app.Activity.getReferrer] are attacker
 * controllable, so they may only exercise non-privileged paths.
 */
enum class DeepLinkOrigin {
    /** Our own notification PendingIntents, or senders holding our signature-level permission. */
    Internal,

    /** Anything else: third-party apps, web browsers, adb. */
    External,
}
