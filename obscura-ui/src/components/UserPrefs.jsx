import { createContext } from 'react';
import { createStorage } from '../tauri/storage';

export const UserPrefsContext = createContext({
    setter: (key, newValueOrHandler) => { },
    loading: true
});

export function UserPrefs({ children }) {
    // system side can get the values of the store pretty easily
    //  but we need to test to confirm
    const userPrefsStorage = createStorage('user-preferences');
    const userPrefsValues = {
        setter: userPrefsStorage.set,
        loading: userPrefsStorage.loading,
        // -------------------------------
        closeTipShown: userPrefsStorage.use('closeTipShown', false)
    };
    return <UserPrefsContext.Provider value={userPrefsValues}>
        {children}
    </UserPrefsContext.Provider>
}
