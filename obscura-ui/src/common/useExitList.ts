import { makeWatchable, useSharedWatchable } from "./useSharedWatchable";
import { getExitList, refreshExitList } from "../bridge/commands";
import { Exit } from "./api";

export interface UseExitListArgs {
  periodS: number,
}

export interface UseExitListResult {
  exitList?: Exit[],
  error?: Error,
}

const EXIT_WATCHABLE = makeWatchable(refreshExitList, getExitList);

export function useExitList({
  periodS,
}: UseExitListArgs): UseExitListResult {
  let r = useSharedWatchable(EXIT_WATCHABLE, periodS);

  return {
    exitList: r.value?.value.exits,
    error: r.error,
  };
}
