# THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!

# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class com.smartbutler.app.* {
  native <methods>;
}

-keep class com.smartbutler.app.WryActivity {
  public <init>(...);

  void setWebView(com.smartbutler.app.RustWebView);
  java.lang.Class getAppClass(...);
  int getId();
  java.lang.String getVersion();
  int startActivity(...);
}

-keep class com.smartbutler.app.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class com.smartbutler.app.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void loadHTMLMainThread(...);
  void evalScript(...);
}

-keep class com.smartbutler.app.RustWebChromeClient,com.smartbutler.app.RustWebViewClient {
  public <init>(...);
}
