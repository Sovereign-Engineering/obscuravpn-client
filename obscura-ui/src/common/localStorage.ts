import { Dispatch, SetStateAction, useEffect, useState } from 'react';

export enum LocalStorageKey {
    CustomApiUrls = "customApiUrls",
    ConnectedBefore = "connected-before",
    LockInPromoSeen = "lock-in-promo-seen",
    LockInBannerSeen = "lock-in-banner-seen",
}

export function useLocalStorage(key: LocalStorageKey, defaultValue: string): [string, Dispatch<SetStateAction<string>>] {
    const [state, setState] = useState(() => localStorageGet(key) || defaultValue);
    useEffect(() => {
        localStorageSet(key, state);
    }, [key, state]);
    return [state, setState];
}

export function getCustomApiUrls(): string[] {
    const customApiUrlsExist = localStorageGet(LocalStorageKey.CustomApiUrls);
    return JSON.parse(customApiUrlsExist ?? '[]');
}

export function setCustomApiUrls(customApiUrls: string[]): string | null {
  return localStorageSet(LocalStorageKey.CustomApiUrls, JSON.stringify(customApiUrls));
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
