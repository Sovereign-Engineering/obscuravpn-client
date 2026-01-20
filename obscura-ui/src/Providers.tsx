import '@fontsource/open-sans';
import { PropsWithChildren } from 'react';
import { MemoryRouter } from 'react-router-dom';
import Mantine from './components/Mantine';

export default function ({ children }: PropsWithChildren) {
  return <>
    {/* Cannot use Browser router for loading from file */}
    <MemoryRouter>
      <Mantine>
        {children}
      </Mantine>
    </MemoryRouter>
  </>;
}
