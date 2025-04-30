import React, { useMemo, useState } from "react";

import { isPromise, normalizeError } from "./utils";

const NEVER_LOADED = 0;

export interface UseAsyncArgs<T> {
    load: () => Promise<T> | T;
    deps?: React.DependencyList;
    returnError?: boolean;
    skip?: boolean;
}

type RefreshCallback<T> = (value?: T, error?: unknown) => void;

export class UseAsyncState<T> {
    value: T | undefined = undefined;
    lastSuccessfulValue: T | undefined = undefined;
    error: Error | undefined = undefined;

    loadVersion: number = NEVER_LOADED;
    valueVersion: number = NEVER_LOADED;

    refreshToken: unknown;
    refreshCallbacks: RefreshCallback<T>[] | undefined;
    refresh?: () => Promise<void>;

    setValue(version: number, value: T, callbacks?: RefreshCallback<T>[]): boolean {
        for (let callback of callbacks ?? []) {
            try {
                callback(value);
            } catch (cbError) {
                console.error("Refresh callback failed", cbError);
            }
        }

        if (version < this.valueVersion) {
            return false;
        }

        this.value = value;
        this.lastSuccessfulValue = value;
        this.error = undefined;
        this.valueVersion = version;

        return true;
    }

    setError(version: number, error: unknown, callbacks?: RefreshCallback<T>[]): boolean {
        for (let callback of callbacks ?? []) {
            try {
                callback(undefined, error);
            } catch (cbError) {
                console.error("Refresh callback failed", cbError);
            }
        }

        if (version < this.loadVersion) {
            return false;
        }

        this.value = undefined;
        this.error = normalizeError(error);
        this.valueVersion = version;

        return true;
    }
}

export interface UseAsyncResult<T> {
    /// The loaded value.
    ///
    /// If `error` or `!everLoaded` this will be undefined.
    ///
    /// Note that this is the **most recently received response** not necessarily the **most recent requested data**. This can occur if multiple versions are requesting data. If you only want to show data from the latest request only show `value` if `!loading`. If you want to always show the most recent available data just show `value` (and maybe show a refreshing indicator if `loading`).
    value: T | undefined,

    /// The value for the current `deps` array.
    ///
    /// This value always corresponds to the most recently requested data and is never stale. If `deps` change this will immediately switch back to `undefined`. Exactly the same as `value` if `!loading`.
    currentValue: T | undefined,

    /// The last successful value if there ever was one.
    lastSuccessfulValue: T | undefined,

    /// An error if it occurred.
    error: Error | undefined,

    /// If this component has ever loaded, successfully or otherwise.
    ///
    /// If true either `value` or `error` will be set (but not necessarily up to date) and the other will be undefined. Note that both will be `undefined` iff `f` successfully returned `undefined`.
    everLoaded: boolean,

    /// The latest version dispatched.
    loadVersion: number,

    /// The version that the current `value` and `error` correspond to.
    valueVersion: number,

    /// If the current values of `value` and `error` do not yet reflect the current `deps`.
    ///
    /// For example the render after `deps` change `value` and `error` will remain the same and `loading` will become `true`. This allows you to either hide the state value, or continue to use it at your discretion.
    loading: boolean,

    refresh: () => Promise<void>,
}

export function useAsync<T>({
    deps = [],
    load,
    returnError = false,
    skip = false,
}: UseAsyncArgs<T>): UseAsyncResult<T> {
    const [state, setState] = useState({
        inner: new UseAsyncState<T>(),
    });
    state.inner.refresh ??= () => {
        return new Promise((resolve, reject) => {
            if (!state.inner.refreshCallbacks) {
                state.inner.refreshToken = {};
                state.inner.refreshCallbacks = [];
            }
            state.inner.refreshCallbacks.push((_, error) => {
                if (error) reject(error);
                else resolve();
            });
            setState({ inner: state.inner });
        })
    };

    useMemo(() => {
        const callbacks = state.inner.refreshCallbacks;
        state.inner.refreshCallbacks = undefined;

        if (skip && !callbacks) {
            return;
        }

        const version = ++state.inner.loadVersion;

        try {
            let r = load();

            if (isPromise(r)) {
                r.then(
                    (value) => {
                        if (state.inner.setValue(version, value, callbacks)) {
                            setState({
                                inner: state.inner,
                            });
                        }
                    },
                    (error) => {
                        if (state.inner.setError(version, error, callbacks)) {
                            setState({
                                inner: state.inner,
                            });
                        }
                    },
                );
            } else {
                state.inner.setValue(version, r, callbacks);
            }
        } catch (error) {
            state.inner.setError(version, error, callbacks);
        }
    }, [skip, state.inner.refreshToken, ...deps]);

    if (state.inner.error && !returnError) {
        throw state.inner.error;
    }

    let loading = state.inner.valueVersion < state.inner.loadVersion;

    return {
        ...state.inner,
        currentValue: loading ? undefined : state.inner.value,
        everLoaded: state.inner.valueVersion > NEVER_LOADED,
        loading,
        refresh: state.inner.refresh!,
    };
}
