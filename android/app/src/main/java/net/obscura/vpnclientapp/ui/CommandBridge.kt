package net.obscura.vpnclientapp.ui

import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.ValueCallback
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.ui.commands.GetOsStatus
import net.obscura.vpnclientapp.ui.commands.GetStatus
import net.obscura.vpnclientapp.ui.commands.InvokeCommand
import net.obscura.vpnclientapp.ui.commands.JsonFfiCommand

class CommandBridge(val eval: (js: String, callback: ValueCallback<String?>?) -> Unit) {

    inline fun logTag(): String {
        return CommandBridge::class.java.name
    }

    @JavascriptInterface
    fun invoke(data: String, id: Int) {
        Log.d(logTag(), "Invoked command ${data} with id ${id}")

        val invokeData = Json.decodeFromString<InvokeCommand>(data)

        eval("""
            (() => {
              "use strict";

              const acceptFn = window.obscuraAndroid.acceptFns.get(${id});
              if (acceptFn) {
                window.obscuraAndroid.acceptFns.remove(${id});
                window.obscuraAndroid.rejectFns.remove(${id});
              }

              acceptFn(${Json.encodeToString(Json.encodeToString(invokeData.run()))});
            })();
        """.trimIndent(), null)
    }

    @JavascriptInterface
    fun initialize() {
        Log.d(logTag(), "Initialize")

        eval(
            """
            (() => {
              "use strict";

              const counterSymbol = Symbol('postMessageCounter');
              window[counterSymbol] = 0;

              const acceptFns = new Map();
              const rejectFns = new Map();

              Object.defineProperty(window, 'obscura', {
                writable: false,
                enumerable: false,
                configurable: false,
                value: Object.freeze({
                  acceptFns: acceptFns,
                  rejectFns: rejectFns,
                })
              });

              Object.defineProperty(window, 'webkit', {
                writable: false,
                enumerable: false,
                configurable: false,
                value: Object.freeze({
                  messageHandlers: Object.freeze({
                    commandBridge: Object.freeze({
                      postMessage: (data) => new Promise((accept, reject) => {
                        const id = window[counterSymbol] += 1;

                        acceptFns[id] = (value) => accept(value);
                        rejectFns[id] = (error) => reject(new Error(error));

                        window.obscuraAndroidCommandBridge.invoke(data, id);
                      })
                    })
                  })
                })
              });

              console.log('onload!!', JSON.stringify(window.webkit.messageHandlers.commandBridge));
            })();
        """.trimIndent(), null)
    }
}
