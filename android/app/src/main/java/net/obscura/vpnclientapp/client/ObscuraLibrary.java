package net.obscura.vpnclientapp.client;

import java.util.concurrent.CompletableFuture;

public class ObscuraLibrary {

  static {
    System.loadLibrary("obscuravpn_client");
  }

  public static native void initialize(String configDir, String userAgent);

  public static native void jsonFfi(String json, CompletableFuture<String> future);
}
