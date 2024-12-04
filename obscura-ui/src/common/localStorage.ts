export const enum LocalStorageKey {
    LastCustomApiUrl = "lastCustomApiUrl"
}

export function localStorageGet(key: LocalStorageKey): string | null {
    return window.localStorage.getItem(key)
}

export function localStorageSet(key: LocalStorageKey, value: string): string | null {
    let prev = localStorageGet(key);
    window.localStorage.setItem(key, value);
    return prev
}

export function localStorageRemove(key: LocalStorageKey): string | null {
    let prev = localStorageGet(key);
    window.localStorage.removeItem(key)
    return prev
}
