import { useSyncExternalStore } from 'react';

export enum LocalStorageKey {
    CustomApiUrls = "customApiUrls",
    ConnectedBefore = "connected-before",
    LockInPromoSeen = "lock-in-promo-seen",
    LockInBannerSeen = "lock-in-banner-seen",
}

const listeners = new Set<() => void>();

export function useLocalStorage(key: LocalStorageKey, defaultValue: string): [string, (value: string) => void] {
    const value = useSyncExternalStore(listener => {
        const subscription = () => listener();
        listeners.add(subscription);
        window.addEventListener('storage', subscription);
        return () => {
            listeners.delete(subscription);
            window.removeEventListener('storage', subscription);
        };
    }, () => getLocalStorage(key) ?? defaultValue);

    return [value, value => setLocalStorage(key, value)];
}

export function getLocalStorage(key: LocalStorageKey) {
    return window.localStorage.getItem(key);
}

export function setLocalStorage(key: LocalStorageKey, value: string) {
    window.localStorage.setItem(key, value);
    listeners.forEach(listener => listener());
}

export function removeLocalStorage(key: string) {
    window.localStorage.removeItem(key);
    listeners.forEach(listener => listener());
}
