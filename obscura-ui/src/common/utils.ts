import Cookies from 'js-cookie';
import localforage from 'localforage';
import { Dispatch, ForwardedRef, RefCallback, SetStateAction, useEffect, useLayoutEffect, useState } from 'react';
import { fmt } from './fmt';
import { useMantineTheme } from '@mantine/core';
export { localforage };

export const HEADER_TITLE = 'Obscura VPN';
export const IS_DEVELOPMENT = import.meta.env.MODE === 'development';
export const MIN_LOAD_MS = 400;

export function useCookie(key: string, defaultValue: string, options: Cookies.CookieAttributes = { expires: 365000, sameSite: 'lax', path: '/' }): [string, Dispatch<SetStateAction<string>>] {
    // cookie expires in a millenia
    // sameSite != 'strict' because the cookie is not read for sensitive actions
    // synchronous
    const cookieValue = Cookies.get(key);
    const [state, setState] = useState(cookieValue || defaultValue);
    useEffect(() => {
        Cookies.set(key, state, options);
    }, [state]);
    return [state, setState];
}

// show browser / native notification
export function notify(title: string, body?: string) {
    new Notification(title, { body: body || "", });
}

export function sleep(ms: number) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

export function downloadFile(filename: string, content: BlobPart, contentType = 'text/plain') {
    const element = document.createElement('a');
    const file = new Blob([content], { type: contentType });
    element.href = URL.createObjectURL(file);
    element.download = filename;
    document.body.appendChild(element); // Required for this to work in FireFox
    element.click();
}

export function isPromise(v: unknown): v is PromiseLike<unknown> {
    return !!v && (typeof v == "object" || typeof v == "function") && "then" in v;
}

export function arraysEqual<T>(a: T[], b: T[]) {
    if (a === b) return true;
    if (a == null || b == null) return false;
    if (a.length !== b.length) return false;

    // If you don't care about the order of the elements inside
    // the array, you should sort both arrays here.
    // Please note that calling sort on an array will modify that array.
    // you might want to clone your array first.

    for (var i = 0; i < a.length; ++i) {
        if (a[i] !== b[i]) return false;
    }
    return true;
}

// https://reactjs.org/docs/hooks-custom.html
export function useLocalForage<T>(key: string, defaultValue: T) {
    // only supports primitives, arrays, and {} objects
    const [state, setState] = useState(defaultValue);
    const [loading, setLoading] = useState(true);

    // useLayoutEffect will be called before DOM paintings and before useEffect
    useLayoutEffect(() => {
        let allow = true;
        localforage.getItem(key)
            .then(value => {
                if (value === null) throw '';
                if (allow) setState(value as T);
            }).catch(() => localforage.setItem(key, defaultValue))
            .then(() => {
                if (allow) setLoading(false);
            });
        return () => { allow = false; }
    }, []);
    // useLayoutEffect does not like Promise return values.
    useEffect(() => {
        // do not allow setState to be called before data has even been loaded!
        // this prevents overwriting
        if (!loading) localforage.setItem(key, state);
    }, [state]);
    return [state, setState, loading];
}

/**
 * A hack to get the latest state value to be used in long running tasks
 * This function should not be made use of liberally
 * @param {A} setter the setState method of the state you want the latest value of
 * @returns the state which was passed to the setter's action
 */
export function getLatestState<S>(setter: Dispatch<SetStateAction<S>>) {
    let v;
    setter(value => {
        v = value;
        return value;
    });
    return v;
}

export function percentEncodeQuery(params: Record<string, string>) {
    return Object.entries(params)
        .map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(value)}`)
        .join('&');
}

const DEFAULT_ERROR_MSG = "An unexpected error has occurred.";

export function errMsg(error: unknown): string {
    if (error instanceof Error) {
        return error.message;
    }
    console.warn(fmt`errMsg: error = ${error} is not an instance of Error`);
    return DEFAULT_ERROR_MSG;
}

export function normalizeError(error: unknown): Error {
    if (error instanceof Error) {
        return error;
    }
    console.warn(fmt`normalizeError: error = ${error} is not an instance of Error`);
    return new Error(DEFAULT_ERROR_MSG, {
        cause: error,
    });
}

export function multiRef<T>(...refs: ForwardedRef<T>[]): RefCallback<T> {
  return value => {
    return refs.forEach((ref) => {
      if (ref !== null) {
        if (typeof ref === 'function') {
          ref(value);
        } else {
          ref.current = value
        }
      }
    });
  };
}

export function randomChoice<T>(arr: T[]): T {
  if (arr.length === 0) throw new Error('array length cannot be zero');
  const randIdx = Math.floor(Math.random() * arr.length);
  return arr[randIdx]!;
}

/**
 * Returns hh:mm:ss from ms
 * @param ms milliseconds
 */
export function fmtTime(ms: number) {
  const totalSeconds = Math.floor(ms / 1000);
  const seconds = totalSeconds % 60;
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const hours = Math.floor(totalSeconds / 3600);
  return `${zeroPad(hours, 2)}:${zeroPad(minutes, 2)}:${zeroPad(seconds, 2)}`;
}

function zeroPad(num: number, width: number) {
  return num.toString().padStart(width, '0');
}

export function usePrimaryColorResolved() {
  const theme = useMantineTheme();
  return theme.variantColorResolver({color: theme.primaryColor, theme, variant: 'subtle'}).color;
}
