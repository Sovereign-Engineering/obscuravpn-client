import { ErrorInfo, useEffect } from 'react';

export const PLATFORM = import.meta.env.OBS_WEB_PLATFORM as Platform;

// Update translation files whenever Platform is updated
export enum Platform {
  macOS = 'macosx',
  iOS = 'iphoneos',
  Android = 'android',
}

export function systemName(): string {
  switch (PLATFORM) {
    case Platform.macOS:
      return "macOS";
    case Platform.iOS:
      return "iOS";
    case Platform.Android:
      return "Android";
  }
}

export const IS_HANDHELD_DEVICE = PLATFORM === Platform.iOS ||
  PLATFORM === Platform.Android;
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
