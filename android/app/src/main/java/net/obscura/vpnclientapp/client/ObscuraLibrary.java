package net.obscura.vpnclientapp.client;

import android.app.Application;
import android.content.Context;
import java.util.concurrent.CompletableFuture;

public class ObscuraLibrary {

  public static void load(Context context, String userAgent) {
    // Using this class outside of the :vpnservice process is not allowed.
    if (Application.getProcessName().endsWith(":vpnservice")) {
      System.loadLibrary("obscuravpn_client");
      initialize(context.getFilesDir().getAbsolutePath(), userAgent);
    }
  }

  public static native void initialize(String configDir, String userAgent);

  public static native void jsonFfi(String json, CompletableFuture<String> future);

  public static native void setNetworkInterfaceIndex(int index);

  public static native void startTunnel(int fd);

  public static native void stopTunnel();
}
