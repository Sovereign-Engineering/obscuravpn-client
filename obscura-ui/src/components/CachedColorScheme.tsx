import { useComputedColorScheme } from '@mantine/core';
import { createContext, PropsWithChildren } from 'react';

// useComputerColorScheme does not return instantly, it first returns the default value
//  and then later returns the true value.
// This causes flashes since the chance from light (default) to dark is noticeable
// To avoid a flash, we can cache the value at a location of the tree where we know
// useComputerColorScheme would be too slow

export const CColorSchemeContext = createContext('light' as 'light' | 'dark');

export default function CachedColorScheme({ children }: PropsWithChildren) {
  const colorScheme = useComputedColorScheme();
  return (
    <CColorSchemeContext.Provider value={colorScheme}>
      {children}
    </CColorSchemeContext.Provider>
  );
}
