import { useComputedColorScheme, useMantineTheme } from '@mantine/core';
import Wordmark from '../res/obscura-wordmark.svg?react';

export default function ObscuraWordmark() {
  const colorScheme = useComputedColorScheme();
  const theme = useMantineTheme();
  return <Wordmark fill={colorScheme === 'light' ? 'black' : theme.colors.gray[4]} width={150} height='auto' />;
}
