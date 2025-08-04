import { ErrorInfo, useEffect } from 'react';

export const PLATFORM = import.meta.env.OBS_WEB_PLATFORM as Platform;

// Update translation files whenever Platform is updated
export enum Platform {
  macOS = 'macosx',
  iOS = 'iphoneos',
}

export const IS_HANDHELD_DEVICE = PLATFORM === Platform.iOS;
const platformDefined = Object.values(Platform).includes(PLATFORM);

export function useSystemChecks() {
  useEffect(() => {
    if (!platformDefined) {
      const errMsg = `OBS_WEB_PLATFORM was unexpected, got "${PLATFORM}"`;
      throw new Error(errMsg);
    }
  }, [platformDefined]);
}

export async function logReactError(error: Error, info: ErrorInfo) {
  console.error(`Render error "${error.message}"; ComponentStack = ${info.componentStack}`);
}
