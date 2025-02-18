import '@fontsource/open-sans';
import { PropsWithChildren } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { SystemProvider } from './bridge/SystemProvider';
import Mantine from './components/Mantine';

export default function ({ children }: PropsWithChildren) {
    return <>
        <SystemProvider>
            {/* In WKWebview, BrowserRouter does not work in production due to sandboxing */}
            <MemoryRouter>
                <Mantine>
                    {children}
                </Mantine>
            </MemoryRouter>
        </SystemProvider>
    </>;
}
