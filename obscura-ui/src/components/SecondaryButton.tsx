import { Button, useComputedColorScheme } from "@mantine/core";
import { PropsWithChildren } from "react";

export function SecondaryButton({ children, onClick }: PropsWithChildren & { onClick: () => void }) {
  const colorScheme = useComputedColorScheme();
  return <Button onClick={onClick} variant='light' color='gray' c={colorScheme === 'light' ? 'gray.7' : 'gray'}>{children}</Button>;
}
