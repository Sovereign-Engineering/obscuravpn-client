import { useMantineColorScheme } from '@mantine/core';
import * as commands from '../bridge/commands';
import { ColorScheme } from './appContext';

/**
 * Informs the native UI of the preferred color scheme for the window and the webview
 * The color scheme is reported in OsStatus.colorScheme
 */
export function useSetAppearance() {
  const { setColorScheme: setMantineColorScheme } = useMantineColorScheme();

  return async (colorScheme: ColorScheme) => {
    try {
      // Backwards compatibility: reset mantine overriding color scheme to use webview's preference
      setMantineColorScheme('auto');
      await commands.setColorScheme(colorScheme);
    } catch (e) {
      console.error('Failed to set theme:', e);
    }
  };
}
