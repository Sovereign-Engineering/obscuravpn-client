import '@fontsource/open-sans';
import { PropsWithChildren } from 'react';
import { MemoryRouter } from 'react-router-dom';
import Mantine from './components/Mantine';

export default function ({ children }: PropsWithChildren) {
  return <>
    {/* In WKWebview, BrowserRouter does not work in production due to sandboxing */}
    <MemoryRouter>
      <Mantine>
        {children}
      </Mantine>
    </MemoryRouter>
  </>;
}
