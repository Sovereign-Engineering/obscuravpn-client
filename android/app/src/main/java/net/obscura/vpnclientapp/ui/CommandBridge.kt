package net.obscura.vpnclientapp.ui

import android.util.Log
import android.webkit.JavascriptInterface
import android.webkit.ValueCallback
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonBuilder
import net.obscura.vpnclientapp.ui.commands.GetOsStatus
import net.obscura.vpnclientapp.ui.commands.GetStatus
import net.obscura.vpnclientapp.ui.commands.InvokeCommand
import net.obscura.vpnclientapp.ui.commands.JsonFfiCommand

class CommandBridge(val eval: (js: String, callback: ValueCallback<String?>?) -> Unit) {

    val json = Json {
        encodeDefaults = true
        ignoreUnknownKeys = true
    }

    inline fun logTag(): String {
        return CommandBridge::class.java.name
    }

    @JavascriptInterface
    fun invoke(data: String, id: Int) {
        Log.d(logTag(), "Invoked command ${data} with id ${id}")

        val invokeData = json.decodeFromString<InvokeCommand>(data)

        try {
            eval(
                """
                (() => {
                  "use strict";

                  window.obscuraAndroidPromises.accept(${id}, ${
                    json.encodeToString(
                        json.encodeToString(
                            invokeData.run()
                        )
                    )
                }
                })()
            """.trimIndent(), null
            )
        } catch (error: Throwable) {
            eval(
                """
                (() => {
                  "use strict";

                  window.obscuraAndroidPromises.reject(${id}, ${
                    json.encodeToString(
                        json.encodeToString(
                            error.message
                        )
                    )
                });
                })()
            """.trimIndent(), null
            )
        }
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

              Object.defineProperty(window, 'obscuraAndroidPromises', {
                writable: false,
                enumerable: false,
                configurable: false,
                value: Object.freeze({
                  accept: (id, value) => {
                    const fn = acceptFns.get(id);
                    if (fn) {
                      fn(value);
                    }
                  },

                  reject: (id, error) => {
                    const fn = rejectFns.get(id);
                    if (fn) {
                      fn(error);
                    }
                  },
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

                        const cleanup = () => {
                          acceptFns.delete(id);
                          rejectFns.delete(id);
                        };

                        acceptFns.set(id, (value) => { cleanup(); accept(value); });
                        rejectFns.set(id, (error) => { cleanup(); reject(new Error(error)); });

                        window.obscuraAndroidCommandBridge.invoke(data, id);
                      })
                    })
                  })
                })
              });

              console.log('onload!!', JSON.stringify(window.webkit.messageHandlers.commandBridge));
            })();
        """.trimIndent(), null
        )
    }
}
