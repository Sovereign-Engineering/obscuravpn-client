import { useEffect, useRef } from "react";
import { useAsync, UseAsyncArgs, UseAsyncResult } from "./useAsync";

export interface UseLoadableArgs<T> extends UseAsyncArgs<T> {
    periodMs: number,
}

export function useLoadable<T>({
    periodMs,
    ...args
}: UseLoadableArgs<T>): UseAsyncResult<T> {
    let failures = useRef(0);

    let r = useAsync(args);

    useEffect(() => {
        // If the dependencies change reset the backoff.
        failures.current = 0;
    }, [args.skip, ...args.deps ?? []]);

    useEffect(() => {
        if (r.loading) {
            // Should only happen for the initial load.
            return;
        }

        let delayMs: number;
        if (r.error) {
            failures.current += 1;
            delayMs = Math.min(
                Math.random() * (500 * 2 ** failures.current),
                60 * 1000,
            );
        } else {
            failures.current = 0;
            delayMs = periodMs;
        }

        let timer = setTimeout(r.refresh, delayMs);
        return () => clearTimeout(timer);
    }, [r.valueVersion])

    return {
        ...r,
        refresh: () => {
            failures.current = 0;
            return r.refresh();
        },
    };
}
