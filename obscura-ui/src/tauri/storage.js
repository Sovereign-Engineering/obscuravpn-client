// API for Tauri or web storage
import localforage from 'localforage';
import { useCallback, useEffect, useRef, useState } from 'react';

const RUNNING_IN_TAURI = window.__TAURI__ !== undefined;
export const USE_STORE = false && RUNNING_IN_TAURI;
// save data after setState
// https://blog.seethis.link/scan-rate-estimator/
const SAVE_DELAY = 400;

// returns an API to get a item, set an item from a specific category of data
// why? we don't to have loading variable for multiple values
export function createStorage(storeName) {
    const localDataRef = useRef();
    const [data, setData] = useState(undefined);
    const loading = data === undefined;
    const fileStoreRef = useRef();
    const timeoutRef = useRef();
    // load data
    useEffect(() => {
        localforage.getItem(storeName, (err, value) => {
            // make store a {} again in catch
            if (err !== undefined && value === null || Array.isArray(value)) {
                localforage.setItem(storeName, {}, (err, val) => {
                    if (err !== null && err !== undefined) {
                        return alert('cannot store data, application will not work as intended');
                    }
                    setData(val);
                });
            } else {
                if (err !== null) {
                    console.log('storage.js: err === null?', err === null);
                }
                setData(value);
            }
        });
    }, []);

    const setItem = useCallback((key, newValueOrHandler) => {
        if (loading) return;
        console.log(newValueOrHandler);
        clearTimeout(timeoutRef.current);
        setData(data => {
            const prev = data[key];
            let newData = data;
            try {
                newData = { ...data, [key]: newValueOrHandler(prev) };
                console.log(JSON.stringify(newData));
            } catch (TypeError) {
                newData = { ...data, [key]: newValueOrHandler };
            }
            if (newData !== data) {
                if (RUNNING_IN_TAURI) {
                    // avoid spiking disk IO by saving every SAVE_DELAY
                    fileStoreRef.current.set('data', newData)
                        .then(() => {
                            timeoutRef.current = setTimeout(() => fileStoreRef.current.save(), SAVE_DELAY)
                        });
                } else {
                    timeoutRef.current = setTimeout(() => localforage.setItem(storeName, newData), SAVE_DELAY);
                }
            }
            return newData;
        });
    }, [loading, fileStoreRef, localDataRef, timeoutRef]);

    const getItem = useCallback((key, defaultValue) => {
        if (loading) return defaultValue;
        const value = data[key];
        if (value === undefined && defaultValue !== undefined) {
            setData(data => ({ ...data, [key]: defaultValue }));
            return defaultValue;
        }
        return value;
    }, [loading, data]);

    const useItem = useCallback((key, defaultValue) => {
        const value = getItem(key, defaultValue);
        return [value, newValue => setItem(key, newValue)];
    }, [getItem, setItem]);

    return {
        get: getItem,
        set: setItem,
        use: useItem,
        data,
        loading
    };
}
