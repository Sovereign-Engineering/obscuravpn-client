import { ActionIcon, Button, Group, Stack, Switch, Text, ThemeIcon, Title, Tooltip, useMantineColorScheme } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useContext, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsCircleHalf } from 'react-icons/bs';
import { IoInformationCircleOutline, IoMoon, IoSunnySharp } from 'react-icons/io5';
import * as commands from '../bridge/commands';
import { AppContext } from '../common/appContext';
import { NotificationId } from '../common/notifIds';
import { useAsync } from '../common/useAsync';
import { errMsg, normalizeError } from '../common/utils';
import { fmtErrorI18n } from '../translations/i18n';
import classes from './Settings.module.css';

export default function Settings() {
    const { t } = useTranslation();
    const { colorScheme, setColorScheme } = useMantineColorScheme();
    const { value: loginItemRegistered, refresh: recheckLoginItem, loading, error: loginItemError } = useAsync({ load: commands.isRegisteredAsLoginItem, returnError: true });
    const { appStatus } = useContext(AppContext);

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
        <Stack gap='xl' align='flex-start' className={classes.container}>
            <Stack gap='lg'>
              <Title order={4}>{t('General')}</Title>
              {
                loginItemError?.message !== 'errorUnsupportedOnOS' &&
                <Switch error={loginItemError === undefined ? undefined : `${loginItemError}`} disabled={loginItemError !== undefined || loading || loginItemRegistered === undefined} checked={loginItemRegistered} onChange={event => event.currentTarget.checked ? registerAtLogin() : unregisterAtLogin()} label={t('openAtLoginRegister')} />
              }
              <Switch checked={appStatus.autoConnect} onChange={event => commands.setAutoConnect(event.currentTarget.checked)} label={t('autoConnectStartup')} />
              <Text size='sm' c='dimmed'>{t('autoConnectStartup-behavior')}</Text>
            </Stack>
            <Stack gap='lg' align='flex-start'>
              <Title order={4}>{t('Network')}</Title>
              <StrictLeakPreventionSwitch />
              <Button onClick={rotateWgKey}>{t('rotateWgKey')}</Button>
            </Stack>
            <Stack gap='lg'>
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
            </Stack>
        </Stack >
    );
}

function StrictLeakPreventionSwitch() {
  const { t } = useTranslation();
  const { vpnConnected, osStatus } = useContext(AppContext);
  const { strictLeakPrevention } = osStatus;

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>();

  const onChange = async (checked: boolean) => {
    try {
      setLoading(true);
      setError(undefined);
      await commands.setStrictLeakPrevention(checked);
    } catch (err) {
      setError(errMsg(err));
    } finally {
      setLoading(false);
    }
  };

  const disabled = strictLeakPrevention && vpnConnected;

  return (
    <Group gap={0}>
      <Switch
        error={error}
        checked={strictLeakPrevention}
        onChange={(event) => onChange(event.currentTarget.checked)}
        disabled={disabled || loading}
        label={t('strictLeakPreventionLabel')}
      />
      {disabled && (
        <Tooltip label={t('strictLeakPreventionTooltip')} withArrow>
          <ThemeIcon variant='transparent' color='gray'>
            <IoInformationCircleOutline size="1.1em" style={{ display: 'block' }} />
          </ThemeIcon>
        </Tooltip>
      )}
    </Group>
  );
}
