import { Button } from '@mantine/core';
import ExternalLinkIcon from './ExternalLinkIcon';

interface ButtonLinkProps {
  href: string,
  text: string
}

export function ButtonLink({ text, href }: ButtonLinkProps) {
  return (
    <Button w={{ base: '100%', xs: 'auto' }} component='a' href={href} size='sm'>
      <span>{text} <ExternalLinkIcon size={11} /></span>
    </Button>
  );
}
