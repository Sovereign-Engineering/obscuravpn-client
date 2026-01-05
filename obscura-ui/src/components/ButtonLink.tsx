import { Button, ButtonProps } from '@mantine/core';
import { PropsWithChildren } from 'react';
import ExternalLinkIcon from './ExternalLinkIcon';

interface ButtonLinkProps extends PropsWithChildren {
  href: string,
  inline?: boolean,
  size?: ButtonProps['size'],
  onClick?: React.MouseEventHandler,
  variant?: ButtonProps['variant'],
}

export function ButtonLink({ children, href, onClick, variant, inline = false, size }: ButtonLinkProps) {
  return (
    <Button
      component='a'
      size={size}
      onClick={onClick}
      variant={variant}
      w={inline ? 'auto' : { base: '100%', xs: 'auto' }}
      display={inline ? 'inline-block' : undefined}
      href={href}
      target='_blank'
    >
      <span>{children} <ExternalLinkIcon size={11} /></span>
    </Button>
  );
}
