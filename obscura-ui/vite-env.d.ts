interface Window {
  webkit: {
    messageHandlers: {
      commandBridge: {
        postMessage(commandJson: string): Promise<string>
      },
      logBridge: {
        postMessage: {
          level: 'log' | 'info' | 'warn' | 'error' | 'debug',
          message: string
        }
      },
      errorBridge: {
        postMessage({
          message,
          source,
          lineno,
          colno,
        }: {
          message: string,
          source: string,
          lineno: number,
          colno: number
        }): void
      }
    }
  },
  // https://learn.microsoft.com/microsoft-edge/webview2/reference/javascript/webview#properties
  chrome: {
    webview: {
      postMessage(message: any) : void,
      addEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | AddEventListenerOptions): void,
      removeEventListener(type: string, listener: EventListenerOrEventListenerObject, options?: boolean | EventListenerOptions): void,
    }
  }
}
