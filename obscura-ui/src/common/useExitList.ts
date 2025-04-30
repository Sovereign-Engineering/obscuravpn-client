import { getExitList, refreshExitList } from "../bridge/commands";
import { Exit } from "./api";
import { useWatchable, UseWatchableResult } from "./useWatchable";

interface Versioned {
  version: unknown
}

export interface UseExitListArgs {
  periodS: number,
}

export interface UseExitListResult {
  exitList?: Exit[],
  error?: Error,
}

export function useExitList({
  periodS,
}: UseExitListArgs): UseExitListResult {
  let r = useWatchable({
    load: refreshExitList,
    periodS,
    watch: getExitList,
  });

  return {
    exitList: r.value?.value.exits,
    error: r.error,
  };
}
