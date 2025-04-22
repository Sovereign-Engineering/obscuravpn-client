import { ActionIcon, Button, Group, Stack, Switch, Text, Title, useMantineColorScheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useTranslation } from 'react-i18next';
import { BsCircleHalf } from 'react-icons/bs';
import { IoMoon, IoSunnySharp } from 'react-icons/io5';
import * as commands from '../bridge/commands';
import { NotificationId } from '../common/notifIds';
import { useAsync } from '../common/useAsync';
import { normalizeError } from '../common/utils';
import { fmtErrorI18n } from '../translations/i18n';

export default function Settings() {
    const { t } = useTranslation();
    const { colorScheme, setColorScheme } = useMantineColorScheme();
    const { value: loginItemRegistered, refresh: recheckLoginItem, loading, error } = useAsync({ load: commands.isRegisteredAsLoginItem, returnError: true });

    const registerAtLogin = async () => {
        let success = true;
        try {
          await commands.registerAsLoginItem();
        } catch {
          success = false;
        } finally {
          await recheckLoginItem();
        }
        notifications.hide(NotificationId.OPEN_AT_LOGIN);
        notifications.show({
            id: NotificationId.OPEN_AT_LOGIN,
            title: success ? t('Success') : t('Failed'),
            message: success ? t('openAtLoginEnabled') : t('openAtLoginFailedToEnable'),
            loading: false,
            color: success ? 'green' : 'red'
        });
    }

    const unregisterAtLogin = async () => {
        let success = true;
        try {
          await commands.unregisterAsLoginItem();
        } catch {
          success = false;
        } finally {
          await recheckLoginItem();
        }
        notifications.hide(NotificationId.OPEN_AT_LOGIN);
        notifications.show({
            id: NotificationId.OPEN_AT_LOGIN,
            title: success ? t('Success') : t('Failed'),
            message: success ? t('openAtLoginDisabled') : t('openAtLoginFailedToDisable'),
            loading: false,
            color: success ? 'green' : 'red'
        });
    }

    const rotateWgKey = async () => {
      try {
        await commands.rotateWgKey();
      } catch (e) {
        const error = normalizeError(e);
        const message = error instanceof commands.CommandError
            ? fmtErrorI18n(t, error) : error.message;
        notifications.show({
            title: t('Error'),
            message: message,
            color: 'red',
        });
      }
    }

    return (
        <Stack gap='lg' align='flex-start' ml={80} mt={40} m={20}>
            <Title order={4}>{t('General')}</Title>
            <Switch error={error === undefined ? undefined : `${error}`} disabled={error !== undefined || loading || loginItemRegistered === undefined} checked={loginItemRegistered} onChange={event => event.currentTarget.checked ? registerAtLogin() : unregisterAtLogin()} label={t('openAtLoginRegister')} />
            <Title order={4}>{t('Appearance')}</Title>
            <Group gap='md'>
                <ActionIcon variant='default' onClick={() => setColorScheme('light')} h={80} w={100} disabled={colorScheme === 'light'}>
                    <Stack align='center' gap='xs'>
                        <IoSunnySharp size='1.5em' />
                        <Text size='sm'>{t('Light')}</Text>
                    </Stack>
                </ActionIcon>
                <ActionIcon variant='default' onClick={() => setColorScheme('dark')} h={80} w={100} disabled={colorScheme === 'dark'}>
                    <Stack align='center' gap='xs'>
                        <IoMoon size='1.25em' />
                        <Text size='sm'>{t('Dark')}</Text>
                    </Stack>
                </ActionIcon>
                <ActionIcon variant='default' onClick={() => setColorScheme('auto')} h={80} w={100} disabled={colorScheme === 'auto'}>
                    <Stack align='center' gap='xs'>
                        <BsCircleHalf style={{ transform: 'rotate(180deg)' }} size='1.25em' />
                        <Text size='sm'>{t('System')}</Text>
                    </Stack>
                </ActionIcon>
            </Group>
            <Button onClick={rotateWgKey}>{t('rotateWgKey')}</Button>
        </Stack >
    );
}
