import { PLATFORM, Platform } from "./SystemProvider";

if (PLATFORM === Platform.Android) {
  const MESSAGE_PREFIX = "android/";
  const NAVIGATE_PREFIX = "android-navigate/";

  let counter = 0;

  const acceptFns = new Map<number, (data: string) => void>();
  const rejectFns = new Map<number, (error: string) => void>();

  window.addEventListener("message", (event) => {
    if (typeof event.data !== "string") {
      return;
    }

    if (
      event.data.startsWith(MESSAGE_PREFIX)
    ) {
      const message: { id: number; error?: string; data?: string } = JSON.parse(
        event.data.substring(MESSAGE_PREFIX.length),
      );

      if (typeof message.error === "string") {
        const reject = rejectFns.get(message.id);
        if (reject) {
          reject(message.error);
        }
      } else if (typeof message.data === "string") {
        const accept = acceptFns.get(message.id);
        if (accept) {
          accept(message.data);
        }
      }
    } else if (event.data.startsWith(NAVIGATE_PREFIX)) {
      window.dispatchEvent(new CustomEvent('navUpdate', {
        detail: event.data.substring(NAVIGATE_PREFIX.length),
      }));
    }
  });

  Object.defineProperty(window, "webkit", {
    writable: false,
    enumerable: false,
    configurable: false,
    value: Object.freeze({
      messageHandlers: Object.freeze({
        commandBridge: Object.freeze({
          postMessage: (data: string) =>
            new Promise((accept, reject) => {
              const id = (counter += 1);

              const cleanup = () => {
                acceptFns.delete(id);
                rejectFns.delete(id);
              };

              acceptFns.set(id, (value) => {
                cleanup();
                accept(value);
              });
              rejectFns.set(id, (error) => {
                cleanup();
                reject(new Error(error));
              });

              // obscuraAndroidCommandBridge is defined by the Android WebView
              (window as any).obscuraAndroidCommandBridge.invoke(data, id);
            }),
        }),
      }),
    }),
  });
}
