package net.obscura.vpnclientapp.client;

import android.app.Application;
import android.content.Context;
import java.util.concurrent.CompletableFuture;

public class ObscuraLibrary {
    static long load(Context context, String userAgent) {
        if (!Application.getProcessName().endsWith(":vpnservice")) {
            throw new IllegalStateException("Using this class outside of the :vpnservice process is not allowed.");
        }
        System.loadLibrary("obscuravpn_client");
        return ObscuraLibrary.initialize(context.getFilesDir().getAbsolutePath(), userAgent);
    }

    static native long initialize(String configDir, String userAgent);

    static native void jsonFfi(long rustFfiContext, String json, CompletableFuture<String> future);

    static native void setNetworkInterface(long rustFfiContext, String name, int index);
    static native void unsetNetworkInterface(long rustFfiContext);

    static native void forwardLog(int level, String tag, String message, String messageId, String throwableString);

    static native void setNetworkConfigDone(long context, int fd);
}
