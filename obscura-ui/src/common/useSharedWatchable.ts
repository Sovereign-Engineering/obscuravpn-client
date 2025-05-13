import { useEffect, useState } from "react";
import { normalizeError, sleep } from "./utils";

interface Versioned {
  version: unknown
}

interface SharedWatchable<T extends Versioned> {
  error: Error | undefined;
  value: T | undefined;

  load: (freshnessS: number) => Promise<void>;
  watch: (version?: T["version"]) => Promise<T>;

  isWatcherRunning: boolean;
  subscribers: Set<(v: UseWatchableResult<T>) => void>;
}

export function makeWatchable<T extends Versioned>(
  load: (freshnessS: number) => Promise<void>,
  watch: (version?: T["version"]) => Promise<T>,
): SharedWatchable<T> {
  return {
    error: undefined,
    value: undefined,

    load,
    watch,

    isWatcherRunning: false,
    subscribers: new Set,
  };
}

export interface UseWatchableResult<T> {
  error?: Error,
  value?: T,
}

export function useSharedWatchable<T extends Versioned>(
  shared: SharedWatchable<T>,
  periodS: number,
): UseWatchableResult<T> {
  let [state, setState] = useState<UseWatchableResult<T>>({
    value: shared.value,
    error: shared.error,
  });

  useEffect(() => {
    let timeout: ReturnType<typeof setTimeout> | undefined;
    void (async function doLoad() {
      timeout = undefined;
      try {
        await shared.load(periodS);
      } catch (error) {
        shared.error = normalizeError(error);
        let r = {
          value: shared.value,
          error: shared.error,
        };
        for (let watcher of shared.subscribers) {
          watcher(r);
        }
      } finally {
        timeout = setTimeout(doLoad, periodS*1000);
      }
    })();

    shared.subscribers.add(setState);

    if (!shared.isWatcherRunning) {
      shared.isWatcherRunning = true;
      void (async () => {
        while (shared.subscribers.size > 0) {
          try {
            let newValue = await shared.watch(shared.value?.version);
            shared.value = newValue;
            let r = {
              value: newValue,
              error: undefined,
            };
            for (let watcher of shared.subscribers) {
              watcher(r);
            }
          } catch (error) {
            console.error("Failure watching value:", error);
            // TODO: Should we report this in some way?
            await sleep(1000);
          }
        }
        shared.isWatcherRunning = false;
      })();
    }

    return () => {
      shared.subscribers.delete(setState);
      if (timeout) {
        clearTimeout(timeout);
      }
    }
  }, []);

  return state;
}
