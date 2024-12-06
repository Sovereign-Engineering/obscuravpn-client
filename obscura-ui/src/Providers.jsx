import '@fontsource/open-sans';
import { MemoryRouter } from 'react-router-dom';
import Mantine from './components/Mantine';
import { SystemProvider } from './tauri/SystemProvider';
import { TitleBar } from './tauri/TitleBar';

export default function ({ children }) {
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
