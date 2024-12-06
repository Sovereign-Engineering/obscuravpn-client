import Cookies from 'js-cookie';
import localforage from 'localforage';
import { Dispatch, SetStateAction, useEffect, useLayoutEffect, useState } from 'react';
export { localforage };

export const HEADER_TITLE = 'Obscura VPN';
export const IS_DEVELOPMENT = import.meta.env.MODE === 'development';
export const IS_WK_WEB_VIEW = window.webkit !== undefined;

export function useCookie(key: string, defaultValue: string, options: Cookies.CookieAttributes = {}): [string, Dispatch<SetStateAction<string>>] {
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
            .then((value: T | null) => {
                if (value === null) throw '';
                if (allow) setState(value);
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

export function normalizeError(error: unknown): Error {
    if (error instanceof Error) {
        return error;
    }

    return new Error(DEFAULT_ERROR_MSG, {
        cause: error,
    });
}
