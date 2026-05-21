import { ErrorInfo, useEffect } from 'react';

export const PLATFORM = import.meta.env.OBS_WEB_PLATFORM as Platform;

// Update translation files whenever Platform is updated
export enum Platform {
  macOS = 'macosx',
  iOS = 'iphoneos',
  Android = 'android',
  Windows = 'windows',
}

export function systemName(): string {
  switch (PLATFORM) {
    case Platform.macOS:
      return "macOS";
    case Platform.iOS:
      return "iOS";
    case Platform.Android:
      return "Android";
    case Platform.Windows:
      return "Windows";
  }
}

export const IS_HANDHELD_DEVICE = PLATFORM === Platform.iOS ||
  PLATFORM === Platform.Android;
const platformDefined = Object.values(Platform).includes(PLATFORM);

// TODO: Can we remove iOS by preventing it from failing early?
// https://linear.app/soveng/issue/OBS-3164/improve-feedback-during-connecting-state
export const CONNECT_REQUIRES_ONLINE = PLATFORM === Platform.iOS || PLATFORM === Platform.macOS;
export const HAS_NE_VPN_STATUS = PLATFORM !== Platform.Windows;

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
