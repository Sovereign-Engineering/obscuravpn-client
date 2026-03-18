package net.obscura.vpnclientapp.client;

import android.app.Application;
import android.content.Context;
import androidx.annotation.Keep;
import java.util.concurrent.CompletableFuture;

public class ObscuraLibrary {
    /** Opaque handle returned by Rust initialization, required by all subsequent native FFI calls. */
    @Keep
    public static class FfiHandle {
        private FfiHandle() {}
    }

    static FfiHandle load(Context context, String userAgent) {
        if (!Application.getProcessName().endsWith(":vpnservice")) {
            throw new IllegalStateException("Using this class outside of the :vpnservice process is not allowed.");
        }
        System.loadLibrary("obscuravpn_client");
        return ObscuraLibrary.initialize(context.getFilesDir().getAbsolutePath(), userAgent);
    }

    static native FfiHandle initialize(String configDir, String userAgent);

    static native void jsonFfi(FfiHandle handle, String json, CompletableFuture<String> future);

    static native void setNetworkInterface(FfiHandle handle, String name, int index);
    static native void unsetNetworkInterface(FfiHandle handle);

    static native void forwardLog(int level, String tag, String message, String messageId, String throwableString);
}
