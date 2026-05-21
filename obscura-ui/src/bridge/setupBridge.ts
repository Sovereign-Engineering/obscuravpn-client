import { PromiseWithResolvers, promiseWithResolvers } from '../common/utils';
import { PLATFORM, Platform } from './SystemProvider';

switch (PLATFORM) {
  case Platform.Windows:
    addBridge(
      'windows/',
      undefined,
      window.chrome.webview.addEventListener.bind(window.chrome.webview),
      (data, id) => {
        window.chrome.webview.postMessage({ data, id });
      },
    );
    break;
  case Platform.Android:
    addBridge(
      'android/',
      'android-navigate/',
      window.addEventListener.bind(window),
      (data, id) => {
        // obscuraAndroidCommandBridge is defined by the Android WebView
        (window as any).obscuraAndroidCommandBridge.invoke(data, id);
      },
    );
    break;
}

const pending = new Map<number, PromiseWithResolvers<string>>();
let counter = 0;

export function addBridge(
  messagePrefix: string,
  navigatePrefix: string | undefined,
  addEventListener: (type: 'message', listener: (event: Event) => void) => void,
  nativePostMessage: (data: string, id: number) => void,
): void {
  addEventListener('message', event => {
    handleMessageEvent(event as MessageEvent, messagePrefix, navigatePrefix);
  });

  Object.defineProperty(window, 'webkit', {
    writable: false,
    enumerable: false,
    configurable: false,
    value: Object.freeze({
      messageHandlers: Object.freeze({
        commandBridge: Object.freeze({
          postMessage: (data: string) => {
            const id = (counter += 1);
            const entry = promiseWithResolvers<string>();
            pending.set(id, entry);
            try {
              nativePostMessage(data, id);
            } catch (err) {
              pending.delete(id);
              entry.reject(err);
            }
            return entry.promise;
          },
        }),
      }),
    }),
  });
}

function handleMessageEvent(
  event: MessageEvent,
  messagePrefix: string,
  navigatePrefix?: string,
): void {
  if (typeof event.data !== 'string') {
    console.warn('got a non-data message event');
    return;
  }

  if (event.data.startsWith(messagePrefix)) {
    const message: { id: number; error?: string; data?: string } = JSON.parse(
      event.data.substring(messagePrefix.length),
    );

    const entry = pending.get(message.id);
    if (!entry) {
      console.error(`could not find bridge promise for message id ${message.id}`);
      return;
    }
    pending.delete(message.id);

    if (typeof message.error === 'string') {
      entry.reject(new Error(message.error));
    } else if (typeof message.data === 'string') {
      entry.resolve(message.data);
    }
  } else if (navigatePrefix !== undefined && event.data.startsWith(navigatePrefix)) {
    window.dispatchEvent(new CustomEvent('navUpdate', {
      detail: event.data.substring(navigatePrefix.length),
    }));
  }
}
