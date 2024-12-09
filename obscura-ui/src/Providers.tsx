import '@fontsource/open-sans';
import { PropsWithChildren } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { SystemProvider } from './bridge/SystemProvider';
import { TitleBar } from './bridge/TitleBar';
import Mantine from './components/Mantine';

export default function ({ children }: PropsWithChildren) {
    return <>
        <SystemProvider>
            {/* In WKWebview, BrowserRouter does not work in production due to sandboxing */}
            <MemoryRouter>
                <Mantine>
                    <TitleBar />
                    {children}
                </Mantine>
            </MemoryRouter>
        </SystemProvider>
    </>;
}
