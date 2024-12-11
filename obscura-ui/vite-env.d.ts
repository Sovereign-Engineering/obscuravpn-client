interface Window {
  webkit?: {
    messageHandlers: {
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
  }
}
