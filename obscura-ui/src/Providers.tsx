import '@fontsource/open-sans';
import '@fontsource/press-start-2p';
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
