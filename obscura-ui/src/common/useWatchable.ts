import { useEffect, useState } from "react";
import { normalizeError, sleep } from "./utils";

interface Versioned {
  version: unknown
}

export interface UseWatchableArgs<T extends Versioned> {
  load: (freshnessS: number) => Promise<void>,
  periodS: number,
  watch: (version?: T["version"]) => Promise<T>,
}

export interface UseWatchableResult<T> {
  error?: Error,
  value?: T,
}

export function useWatchable<T extends Versioned>({
  load,
  periodS,
  watch,
}: UseWatchableArgs<T>): UseWatchableResult<T> {
  let [state, setState] = useState<UseWatchableResult<T>>({});

  useEffect(() => {
    async function doLoad() {
      try {
        await load(periodS);
        setState(state => {
          if (!state.error) return state;

          return {
            ...state,
            error: undefined,
          };
        });
      } catch (error) {
        setState(state => ({
          ...state,
          error: normalizeError(error),
        }));
      }
    }
    let interval = setInterval(doLoad, periodS*1000);

    let watching = true;
    let value: T | undefined;
    void (async () => {
      while (watching) {
        try {
          let newValue = await watch(value?.version);
          if (newValue.version === value?.version) continue;

          value = newValue;
          setState(state => ({
            ...state,
            value,
          }));
        } catch (error) {
          console.error("Failure watching value:", error);
          // TODO: Should we report this in some way?
          await sleep(1000);
        }
      }
    })();

    return () => {
      clearInterval(interval);
      watching = false;
    }
  }, []);

  return state;
}
