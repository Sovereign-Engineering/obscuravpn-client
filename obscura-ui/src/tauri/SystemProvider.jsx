import * as fs from '@tauri-apps/api/fs';
import * as os from '@tauri-apps/api/os';
import * as tauriPath from '@tauri-apps/api/path';
import { currentMonitor, getCurrent } from '@tauri-apps/api/window';
import { createContext, useContext, useEffect, useState } from 'react';
import { IS_WK_WEB_VIEW } from '../common/utils';

export const APP_NAME = 'Obscura VPN';
export const RUNNING_IN_TAURI = window.__TAURI__ !== undefined;
const EXTS = new Set(['.json']);

// NOTE: Add memoized Tauri calls in this file
//   that you want to use synchronously across components in your app

// defaults are only for auto-complete
const SystemContext = createContext({
  BUNDLE_ID: 'Obscura-VPN',
  // currently, only loading when using Tauri
  loading: RUNNING_IN_TAURI,
  downloads: undefined,
  documents: undefined,
  appDocuments: undefined,
  osType: undefined,
  osPlatform: undefined,
  fileSep: '/',
  isFullScreen: false,
  usingCustomTitleBar: false,
  logDir: undefined,
  defaultLogFile: undefined,
  scaleFactor: 1
});


export const useSystemContext = () => useContext(SystemContext);
export function SystemProvider({ children }) {
  const contextValues = {
    loading: false,
    fileSep: "/",
    downloads: undefined,
    documents: undefined,
    osType: "darwin",
    osPlatform: "Darwin",
    appDocuments: undefined,
    isFullScreen: undefined,
    usingCustomTitleBar: false,
    logDir: undefined,
    defaultLogFile: undefined,
    scaleFactor: 1
  }


  return <SystemContext.Provider value={contextValues}>
    {children}
  </SystemContext.Provider>;
}

export function useMinWidth(minWidth) {
  if (RUNNING_IN_TAURI) {
    useEffect(() => {
      async function resizeWindow() {
        // to set a size consistently across devices,
        //  one must use LogicalSize (Physical cannot be relied upon)
        const physicalSize = await getCurrent().innerSize();
        // Since innerSize returns Physical size, we need
        //   to get the current monitor scale factor
        //   to convert the physical size into a logical size
        const monitor = await currentMonitor();
        const scaleFactor = monitor.scaleFactor;
        const logicalSize = physicalSize.toLogical(scaleFactor);
        if (logicalSize.width < minWidth) {
          logicalSize.width = minWidth;
          await getCurrent().setSize(logicalSize);
        }
      }
      resizeWindow().catch(console.error);
    }, []); // [] to ensure on first render
  }
}

export async function getUserAppFiles() {
  // returns an array of files from $DOCUMENT/$APP_NAME/* with extension that is in EXTS
  //  implying that the app (tauri-plugin-store) can parse the files
  // returns [] if $DOCUMENT/$APP_NAME is a file
  const documents = await tauriPath.documentDir();
  const saveFiles = [];
  await fs.createDir(APP_NAME, { dir: fs.BaseDirectory.Document, recursive: true });
  const entries = await fs.readDir(APP_NAME, { dir: fs.BaseDirectory.AppData, recursive: true });
  if (entries !== null) {
    const osType = await os.type();
    const sep = osType === 'Windows_NT' ? '\\' : '/'
    const appFolder = `${documents}${sep}${APP_NAME}`;
    for (const { path } of flattenFiles(entries)) {
      const friendlyName = path.substring(appFolder.length + 1, path.length);
      if (EXTS.has(getExtension(path).toLowerCase())) saveFiles.push({ path, name: friendlyName });
    }
  }
  return saveFiles;
}

function* flattenFiles(entries) {
  // takes a tree of files and dirs and yields only the files
  for (const entry of entries) {
    entry.children === null ? yield entry.path : yield* fileSaveFiles(entry.children);
  }
}

function getExtension(path) {
  // Modified from https://stackoverflow.com/a/12900504/7732434
  // get filename from full path that uses '\\' or '/' for seperators
  var basename = path.split(/[\\/]/).pop(),
    pos = basename.lastIndexOf('.');
  // if `.` is not in the basename
  if (pos < 0) return '';
  // extract extension including `.`
  return basename.slice(pos);
}


export async function tauriLogError(error, info) {
  console.error(`Render error "${error.message}"; ComponentStack = ${info.componentStack}`);
}
