// easy way for user to view and get the log file
import { ActionIcon, Button, Code, Group, Loader, Space, Textarea, Title, useMantineTheme } from '@mantine/core';
import { useInterval } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { appWindow } from '@tauri-apps/api/window';
import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { IoArrowDown } from 'react-icons/io5';
import { useSystemContext } from '../tauri/SystemProvider';
import * as commands from '../tauri/commands';

export default function LogViewer() {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    const { osPlatform, loading: tauriLoading, defaultLogFile } = useSystemContext();

    const [logContents, setLogContents] = useState('');
    const [firstLogRead, setFirstLogRead] = useState(false);

    const scrollToBottom = () => {
        if (lastElRef.current === undefined || lastElRef.current === 'STB') {
            lastElRef.current = 'STB';
        } else {
            lastElRef.current.scrollIntoView();
        }
    }

    const readLogFile = async () => {
        if (appWindow.isVisible()) {
            // don't want to needlessly read disk when app is only running in tray
            try {
                setLogContents(await commands.readContents(defaultLogFile));
            } catch (e) {
                notifications.show({ title: t('Error'), message: e.message });
            }
        }
    }

    const interval = useInterval(readLogFile, 1000);

    useEffect(() => {
        if (!tauriLoading) {
            readLogFile().then(() => {
                setFirstLogRead(true);
                scrollToBottom();
            });
        }
        interval.start();
        return interval.stop;
    }, [tauriLoading]);


    const loading = !firstLogRead || tauriLoading;

    useEffect(() => {
        interval.start();
        return interval.stop;
    }, []);

    const lastElRef = useRef();

    return loading ? <Loader size='xl' /> : <>
        <Title order={4}>{t('Logs')} </Title>
        <Space />
        <Group justify='space-between'>
            <Group>
                <Button onClick={() => commands.showItemInFolder(defaultLogFile)}>{t(`revealFile_${osPlatform}`)}</Button>
                <Code>{defaultLogFile}</Code>
            </Group>
            <ActionIcon size='lg' onClick={scrollToBottom} title={t('Scroll to bottom')} color={theme.primaryColor} variant='filled'>
                <IoArrowDown size={25} />
            </ActionIcon>
        </Group>
        <Space />
        {
            logContents === '' ? t('[LOG FILE IS EMPTY]') :
                <Textarea autosize value={logContents} />
        }
        {/* footer div */}
        <div ref={el => {
            const stb = lastElRef.current === 'STB';
            lastElRef.current = el
            if (stb) scrollToBottom();
        }}></div>
    </>;
}
