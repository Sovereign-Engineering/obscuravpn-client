import { Anchor, Button, Center, Flex, Group, Image, Loader, Modal, Space, Stack, Text, ThemeIcon, Title } from '@mantine/core';
import { useThrottledValue } from '@mantine/hooks';
import { lazy, Suspense, useContext, useEffect, useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { FaCheckCircle, FaExclamationTriangle } from 'react-icons/fa';
import AppIcon from '../../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_512x512@2x@2x.png';
import * as commands from '../bridge/commands';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import { LEGAL_WEBPAGE, OBSCURA_WEBPAGE } from '../common/accountUtils';
import { AppContext, UpdaterStatusType } from '../common/appContext';
import { MIN_LOAD_MS } from '../common/utils';
import LogsCapture from '../components/LogCapture';
import ObscuraWordmark from '../components/ObscuraWordmark';
import { Socials } from '../components/Socials';
import classes from './About.module.css';

const Licenses = lazy(() => import('../components/Licenses'));

export default function About() {
  const { t } = useTranslation();
  const { osStatus } = useContext(AppContext);
  const { updaterStatus } = osStatus;
  const [showLicenses, setShowLicenses] = useState(false);
  const handleCommand = commands.useHandleCommand(t);

  useEffect(() => {
    // Intentionally run only on mount (recheck once if update not already available)
    if (updaterStatus?.type !== UpdaterStatusType.Available && !IS_HANDHELD_DEVICE) {
      handleCommand(commands.checkForUpdates);
    }
  }, []);
  const updaterStatusDelayed = useThrottledValue(updaterStatus, updaterStatus.type === UpdaterStatusType.Initiated ? MIN_LOAD_MS : 0);
  const isLatest = errorCodeIsLatestVersion(updaterStatusDelayed.errorCode);
  return (
    <Flex className={classes.container} gap='md' direction='column' justify='flex-end' h='100%' >
      <Stack align='center' style={{ flexGrow: '1' }} justify='space-around'>
        <Stack align='center'>
          <Image src={AppIcon} w={120} />
          <ObscuraWordmark />
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
            {!IS_HANDHELD_DEVICE && <> {
              updaterStatusDelayed?.type === UpdaterStatusType.Available ? (
                <Button onClick={() => handleCommand(commands.installUpdate)}>{t('installUpdate')}</Button>
              ) : (
                <Button onClick={() => handleCommand(commands.checkForUpdates)}>{t('checkForUpdates')}</Button>
              )
            }</>}
          </Group>
        </Stack>
      </Stack>
      {IS_HANDHELD_DEVICE && <Stack gap='lg' p='md'>
        <LogsCapture osStatus={osStatus} />
        <Socials />
      </Stack>}
      <Stack mb={10} align='center' ta='center'>
        <Text c='dimmed'>{t('copyright')}</Text>
        <Modal opened={showLicenses} onClose={() => setShowLicenses(false)} size='lg' mt={IS_HANDHELD_DEVICE ? 20 : undefined} title={<Title order={3}>{t('openSourceLicenses')}</Title>}>
          <Suspense fallback={<Center><Loader type='bars' size='md' /></Center>}>
            <Stack>
              <Licenses />
            </Stack>
          </Suspense>
        </Modal>
        <Text><Trans i18nKey='visitObscura' components={[<Anchor href={OBSCURA_WEBPAGE} />]} /></Text>
        <Flex className={classes.ossLicensesGroup} gap='md'>
          <Anchor onClick={() => setShowLicenses(o => !o)}>
            {t('openSourceLicenses')}
          </Anchor>
          <Anchor href={LEGAL_WEBPAGE}>{t('tosAndPrivacyPolicy')}</Anchor>
        </Flex>
        <Text c='dimmed'>{<Trans i18nKey='WGTrademark' components={[<Text component='span' display='inline-block' />]} />}</Text>
        {IS_HANDHELD_DEVICE && <Space h='md' />}
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
