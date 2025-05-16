import { Anchor, Button, Center, Flex, Group, Image, Loader, Modal, Stack, Text, ThemeIcon, Title, useComputedColorScheme, useMantineTheme } from '@mantine/core';
import { useThrottledValue } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { lazy, Suspense, useContext, useEffect, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_512x512@2x@2x.png';
import * as commands from '../bridge/commands';
import { IS_MOBILE } from '../bridge/SystemProvider';
import { LEGAL_WEBPAGE, OBSCURA_WEBPAGE } from '../common/accountUtils';
import { AppContext, UpdaterStatusType } from '../common/appContext';
import { MIN_LOAD_MS, normalizeError } from '../common/utils';
import Wordmark from '../res/obscura-wordmark.svg?react';
import { fmtErrorI18n } from '../translations/i18n';
import classes from './About.module.css';

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
    } catch (e) {
      const error = normalizeError(e);
      const message = error instanceof commands.CommandError
        ? fmtErrorI18n(t, error) : error.message;
      notifications.show({
        color: 'red',
        title: t('Error'),
        message
      });
    }
  };

  useEffect(() => {
    // Intentionally run only on mount (recheck once if update not already available)
    if (updaterStatus?.type !== UpdaterStatusType.Available && !IS_MOBILE) {
      handleCommand(commands.checkForUpdates);
    }
  }, []);
  const updaterStatusDelayed = useThrottledValue(updaterStatus, updaterStatus.type === UpdaterStatusType.Initiated ? MIN_LOAD_MS : 0);
  const isLatest = errorCodeIsLatestVersion(updaterStatusDelayed.errorCode);
  return (
    <Flex gap='md' className={classes.container}>
      <Stack gap='lg' align='center' m={60}>
        <Image src={AppIcon} w={120} />
        <Wordmark fill={colorScheme === 'light' ? 'black' : theme.colors.gray[4]} width={150} height='auto' />
        <Group gap={0}>
          {isLatest && <ThemeIcon variant='transparent' c='green.8'><FaCheckCircle /></ThemeIcon>}
          {updaterStatusDelayed.type === UpdaterStatusType.Available && <ThemeIcon variant='transparent' c='yellow'><FaExclamationTriangle /></ThemeIcon>}
          {updaterStatusDelayed.type === UpdaterStatusType.Initiated && <Loader size='xs' mr='xs' />}
          <Text>
            {osStatus.srcVersion}
            {isLatest && <> ({t('latestVersion')})</>}
            {updaterStatusDelayed.type === UpdaterStatusType.Available && <> ({t('updateAvailable', { version: updaterStatusDelayed.appcast!.version })})</>}
          </Text>
        </Group>
        {(updaterStatusDelayed.type === UpdaterStatusType.NotFound || updaterStatusDelayed.type == UpdaterStatusType.Error) && (
          <UpdaterError errorCode={updaterStatusDelayed.errorCode} error={updaterStatusDelayed.error!} />
        )}
        <Group>
          <Button component='a' href={OBSCURA_WEBPAGE} variant='outline'>{t('Website')}</Button>
          {!IS_MOBILE && <> {
            updaterStatusDelayed?.type === UpdaterStatusType.Available ? (
              <Button onClick={() => handleCommand(commands.installUpdate)}>{t('installUpdate')}</Button>
            ) : (
              <Button onClick={() => handleCommand(commands.checkForUpdates)}>{t('checkForUpdates')}</Button>
            )
          }</>}
        </Group>
      </Stack>
      <Stack mb={10} align='center' ta='center'>
        <Text c='dimmed'>{t('copyright')}</Text>
        <Modal opened={showLicenses} onClose={() => setShowLicenses(false)} size='lg' title={<Title order={3}>{t('openSourceLicenses')}</Title>}>
          <Suspense fallback={<Center><Loader type='bars' size='md' /></Center>}>
            <Stack>
              <Licenses />
            </Stack>
          </Suspense>
        </Modal>
        <Flex className={classes.ossLicensesGroup} gap='md'>
          <Anchor onClick={() => setShowLicenses(o => !o)}>
            {t('openSourceLicenses')}
          </Anchor>
          <Anchor href={LEGAL_WEBPAGE}>{t('tosAndPrivacyPolicy')}</Anchor>
        </Flex>
        <Text c='dimmed'>{<Trans i18nKey='WGTrademark' components={[<Text component='span' display='inline-block' />]} />}</Text>
      </Stack>
    </Flex>
  );
}

interface UpdaterErrorProps {
  errorCode?: number,
  error: string
}

function errorCodeIsLatestVersion(errorcode: number | undefined) {
  return errorcode === 1 || errorcode == 2;
}

function UpdaterError({ errorCode, error }: UpdaterErrorProps) {
  switch (errorCode) {
    // Project version matches
    case 1:
    // Project version exceeds the update
    case 2:
      return null;
    default:
      return <Text c='red'>{error}</Text>;
  }
}
