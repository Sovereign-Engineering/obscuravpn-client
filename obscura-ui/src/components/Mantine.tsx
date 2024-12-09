// boilerplate components
// core styles are required for all packages
import '@mantine/core/styles.css';
import '@mantine/notifications/styles.css';
// other css files are required only if
// you are using components from the corresponding package
// import '@mantine/dates/styles.css';
// import '@mantine/dropzone/styles.css';
// import '@mantine/code-highlight/styles.css';
import { ColorSchemeScript, MantineProvider, createTheme } from '@mantine/core';
import { ModalsProvider } from '@mantine/modals';
import { Notifications } from '@mantine/notifications';
import { PropsWithChildren } from 'react';

export default function Mantine({ children }: PropsWithChildren) {
    // override theme for Mantine (default props and styles)
    // https://mantine.dev/theming/mantine-provider/
    const theme = createTheme({
        fontFamily: '-apple-system, BlinkMacSystemFont, Segoe UI Variable Text, Segoe UI, Roboto, Helvetica, Arial, sans-serif, Apple Color Emoji, Segoe UI Emoji',
        fontFamilyMonospace: 'source-code-pro, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, Liberation Mono, Courier New, monospace',
        // for each component's mantine docs, "Styles API" contains the inner elements that are available to style
        components: {
            Checkbox: { styles: { input: { cursor: 'pointer' }, label: { cursor: 'pointer' } } },
            TextInput: { styles: { label: { marginTop: '0.5rem' } } },
            Select: { styles: { label: { marginTop: '0.5rem' } } },
            Loader: { defaultProps: { size: 'xl' } },
            Space: { defaultProps: { h: 'sm' } },
            Anchor: { defaultProps: { target: '_blank' } },
            Burger: { styles: { burger: { color: '--mantine-color-grey-6' } } },
            CopyButton: { defaultProps: { timeout: 1100 } },
            Button: {
                defaultProps: {
                    radius: 'md',
                    variant: 'gradient',
                },
            }
        },
        primaryColor: 'orange',
        // see figma design for buttons
        defaultGradient: { from: '#FF7A49', to: '#FF6025', deg: 180 },
        // Mantine v7 has ugly dark colors. Therefore, use colors from v6 (https://v6.mantine.dev/theming/colors/#default-colors)
        colors: {
            // dark.4 is the borderColor for dark appearance
            dark: ['#C1C2C5', '#A6A7AB', '#909296', '#5c5f66', '#4F5156', '#393939', '#353535', '#313131', '#303030', '#222528'],
        },
        other: {
            buttonDisconnectProps: { variant: 'light', c: 'red.7', bg: 'red.1' }
        }
    });

    return <>
        <ColorSchemeScript defaultColorScheme='auto' />
        <MantineProvider defaultColorScheme='auto' theme={theme}>
            <ModalsProvider>
                <Notifications />
                {children}
            </ModalsProvider>
        </MantineProvider>
    </>
}
