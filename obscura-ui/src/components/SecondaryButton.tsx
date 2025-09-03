import { Button } from '@mantine/core';
import { PropsWithChildren } from 'react';
import commonClasses from '../common/common.module.css';

export function SecondaryButton({ children, onClick }: PropsWithChildren & { onClick: () => void }) {
  return <Button onClick={onClick} variant='light' color='gray' className={commonClasses.secondaryColor}>{children}</Button>;
}
