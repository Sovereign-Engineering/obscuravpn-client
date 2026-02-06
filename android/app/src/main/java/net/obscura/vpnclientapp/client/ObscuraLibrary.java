package net.obscura.vpnclientapp.client;

import android.app.Application;
import android.content.Context;
import java.util.concurrent.CompletableFuture;

public class ObscuraLibrary {
    private static boolean isLoaded = false;

    public static void load(Context context, String userAgent) {
        // Using this class outside of the :vpnservice process is not allowed.
        if (Application.getProcessName().endsWith(":vpnservice")) {
            System.loadLibrary("obscuravpn_client");
            ObscuraLibrary.initialize(context.getFilesDir().getAbsolutePath(), userAgent);
            ObscuraLibrary.isLoaded = true;
        }
    }

    public static boolean getIsLoaded() {
        return ObscuraLibrary.isLoaded;
    }

    public static native void initialize(String configDir, String userAgent);

    public static native void jsonFfi(String json, CompletableFuture<String> future);

    public static native void setNetworkInterfaceIndex(int index);

    public static native void startTunnel(int fd);

    public static native void stopTunnel();

    public static native void forwardLog(int level, String tag, String message, String messageId, String throwableString);
}
