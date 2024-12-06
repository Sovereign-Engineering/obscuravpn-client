import { ActionIcon, Button, Group, Stack, Text, Title, useComputedColorScheme, useMantineColorScheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { t } from 'i18next';
import { useState } from 'react';
import { BsMoonStarsFill } from 'react-icons/bs';
import { IoSunnySharp } from 'react-icons/io5';
import * as commands from '../bridge/commands';
import { useSystemContext } from '../bridge/SystemProvider';
import AnimatedChevron from '../components/AnimatedChevron';
import DebuggingArchive from '../components/DebuggingArchive';
import Licenses from '../components/Licenses';

export default function Settings() {
    // null (clean ready state) | in-progress | error(msg) (retry state)
    const { osPlatform } = useSystemContext();
    const { toggleColorScheme } = useMantineColorScheme();
    const colorScheme = useComputedColorScheme();
    const [showLicenses, setShowLicenses] = useState(false);


    const registerAtLogin = async () => {
        notifications.show({
            id: 'LOGIN_ITEM',
            title: t('registeringAsLoginItem'),
            loading: true,
            color: 'yellow',
            autoClose: false
        });
        let success = await commands.registerLoginItem();

        notifications.update({
            id: 'LOGIN_ITEM',
            title: success ? t('Success') : t('Failed'),
            message: success ? t('registeredAtLogin') : t('registerLoginItemFailed'),
            loading: false,
            color: success ? 'green' : 'red',
            autoClose: 7000
        });
    }

    return (
        <Stack gap='lg' align='flex-start' p={20}>
            <Group gap={10}>
                <ActionIcon id='toggle-theme' title={osPlatform === 'darwin' ? '⌘ + J' : 'ctrl + J'} variant='default' onClick={() => toggleColorScheme()} size='xl'>
                    {colorScheme === 'dark' ? <IoSunnySharp size='1.5em' /> : <BsMoonStarsFill />}
                </ActionIcon>
                <Text fz='xl'>{t('toggleTheme')}</Text>
            </Group>
            <DebuggingArchive />
            <Button onClick={registerAtLogin}>{t('registerLoginItem')}</Button>
            <Group>
                <Title order={1}>{t('openSourceLicenses')}</Title>
                <Button onClick={() => setShowLicenses(o => !o)} miw={100}>
                    <Group gap='xs'>{showLicenses ? t('Hide') : t('Show')}<AnimatedChevron rotated={showLicenses} /></Group>
                </Button>
            </Group>
            { /* Don't load license file until need to show */}
            {showLicenses && <Licenses />}
        </Stack>
    );
}
