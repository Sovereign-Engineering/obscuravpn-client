import { Anchor, Button, Center, Group, Image, Loader, Modal, Stack, Text, ThemeIcon, Title, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { useThrottledValue } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { lazy, Suspense, useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_512x512@2x@2x.png';
import * as commands from '../bridge/commands';
import { LEGAL_WEBPAGE, OBSCURA_WEBPAGE } from '../common/accountUtils';
import { AppContext, UpdaterStatusType } from '../common/appContext';
import { tUnsafe } from '../common/danger';
import { MIN_LOAD_MS, normalizeError } from '../common/utils';
import Wordmark from '../res/obscura-wordmark.svg?react';

const Licenses = lazy(() => import('../components/Licenses'));

export default function About() {
  const { t } = useTranslation();
  const theme = useMantineTheme();
  const colorScheme = useComputedColorScheme();
  const { osStatus } = useContext(AppContext);
  const { updaterStatus } = osStatus;
  const [showLicenses, setShowLicenses] = useState(false);

  const handleCommand = async (command: () => Promise<void> | void) => {
    try {
      await command();
    } catch (error) {
      notifications.show({
        color: 'red',
        title: t('Error'),
        message: tUnsafe(t, normalizeError(error).message)
      });
    }
  };

  useEffect(() => {
    // Intentionally run only on mount (recheck once if update not already available)
    if (updaterStatus?.type !== UpdaterStatusType.Available) {
      handleCommand(() => commands.checkForUpdates());
    }
  }, []);
  const updaterStatusDelayed = useThrottledValue(updaterStatus, updaterStatus.type === UpdaterStatusType.Initiated ? MIN_LOAD_MS : 0);

  return (
    <Stack justify='space-between' h='100vh'>
      <Stack gap='lg' align='center' m={60}>
        <Image src={AppIcon} w={120} />
        <Wordmark fill={colorScheme === 'light' ? 'black' : theme.colors.gray[4]} width={150} height='auto' />
        <Group gap={0}>
          {updaterStatusDelayed.errorCode === 2 && <ThemeIcon variant='transparent' c='green.8'><FaCheckCircle /></ThemeIcon>}
          {updaterStatusDelayed.type === UpdaterStatusType.Available && <ThemeIcon variant='transparent' c='yellow'><FaExclamationTriangle /></ThemeIcon>}
          {updaterStatusDelayed.type === UpdaterStatusType.Initiated && <Loader size='xs' mr='xs' />}
          <Text>
            {osStatus.srcVersion}
            {updaterStatusDelayed.errorCode === 2 && <> ({t('latestVersion')})</>}
            {updaterStatusDelayed.type === UpdaterStatusType.Available && <> ({t('updateAvailable', { version: updaterStatusDelayed.appcast!.version })})</>}
          </Text>
        </Group>
        {(updaterStatusDelayed.type === UpdaterStatusType.NotFound || updaterStatusDelayed.type == UpdaterStatusType.Error) && (
          <UpdaterError errorCode={updaterStatusDelayed.errorCode} error={updaterStatusDelayed.error!} />
        )}
        <Group>
          <Button component='a' href={OBSCURA_WEBPAGE} variant='outline'>{t('Website')}</Button>
          {
            updaterStatusDelayed?.type === UpdaterStatusType.Available ? (
              <Button onClick={() => handleCommand(commands.installUpdate)}>{t('installUpdate')}</Button>
            ) : (
              <Button onClick={() => handleCommand(commands.checkForUpdates)}>{t('checkForUpdates')}</Button>
            )
          }
        </Group>
      </Stack>
      <Stack m={40} align='center'>
        <Text c='dimmed'>{t('copyright')}</Text>
        <Modal opened={showLicenses} onClose={() => setShowLicenses(false)} size='lg' title={<Title order={3}>{t('openSourceLicenses')}</Title>}>
          <Suspense fallback={<Center><Loader type='bars' size='md' /></Center>}>
            <Stack>
              <Licenses />
            </Stack>
          </Suspense>
        </Modal>
        <Group>
          <Anchor onClick={() => setShowLicenses(o => !o)}>
            {t('openSourceLicenses')}
          </Anchor>
          <Anchor href={LEGAL_WEBPAGE}>{t('tosAndPrivacyPolicy')}</Anchor>
        </Group>
      </Stack>
    </Stack>
  );
}

interface UpdaterErrorProps {
  errorCode?: number,
  error: string
}

function UpdaterError({ errorCode, error }: UpdaterErrorProps) {
  switch (errorCode) {
    case 2:
      return null
    default:
      return <Text c='red'>{error}</Text>;
  }
}
