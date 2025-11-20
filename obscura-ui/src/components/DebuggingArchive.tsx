import { Anchor, Button, Card, Group, Loader, Space, Stack, Text, Textarea, Title } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { IoIosMail, IoIosShare } from 'react-icons/io';
import * as commands from '../bridge/commands';
import { emailDebugArchive, revealItemInDir, shareDebugArchive } from '../bridge/commands';
import { IS_HANDHELD_DEVICE, systemName } from '../bridge/SystemProvider';
import { NEVPNStatus, OsStatus } from '../common/appContext';
import { useDebuggingArchive } from '../common/debuggingArchiveHook';
import { EMAIL } from '../common/links';
import useMailto from '../common/useMailto';
import { ConfirmationDialog } from './ConfirmationDialog';
import classes from './DebuggingArchive.module.css';

const ICON_SIZE = 20;

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function DebuggingArchive({ osStatus }: { osStatus: OsStatus }) {
  const { t } = useTranslation();
  const mailto = useMailto(osStatus);
  const createDebuggingArchive = useDebuggingArchive();
  const [opened, { open, close }] = useDisclosure(false);
  const commandHandler = commands.useHandleCommand(t);
  const [disconnectInProgress, setDisableButtons] = useState(false);
  const [userFeedback, setUserFeedback] = useState('');

  const onContinue = () => {
    setDisableButtons(false);
    void createDebuggingArchive(userFeedback);
    close();
    setUserFeedback('');
  }

  const modalOpen = opened && !osStatus.debugBundleStatus.inProgress;

  const modal = (
    <ConfirmationDialog title={t('Debugging Archive')} opened={modalOpen} onClose={close}>
      <Stack h='100%' justify='space-between' gap='xs'>
        {
          osStatus.osVpnStatus !== NEVPNStatus.Disconnected
          && <>
            <Text>{t('debugArchiveDisconnectPrompt')}</Text>
          </>
        }
        <Textarea
          data-autofocus
          label={t('debugArchiveFeedbackLabel')}
          placeholder={t('debugArchiveFeedbackPrompt')}
          value={userFeedback}
          onChange={(event) => setUserFeedback(event.currentTarget.value)}
          minRows={3}
          maxRows={6}
        />
        <Group w='100%' grow>
          <Button disabled={disconnectInProgress} miw={130} onClick={onContinue} variant='light'>{
            osStatus.osVpnStatus === NEVPNStatus.Disconnected ?
              t('Continue') : t('Stay Connected')
          }</Button>
          {
            osStatus.osVpnStatus !== NEVPNStatus.Disconnected &&
            <Button disabled={disconnectInProgress} miw={130} onClick={async () => {
              setDisableButtons(true);
              await commandHandler(commands.disconnect);
              let knownOsStatusId = null;
              while (true) {
                const newOsStatus = commands.osStatus(knownOsStatusId);
                if ((await newOsStatus).osVpnStatus === NEVPNStatus.Disconnected) {
                  break;
                }
              }
              onContinue();
            }}>{
                disconnectInProgress ? <Loader size={ICON_SIZE} /> : t('Disconnect')}</Button>
          }
        </Group>
      </Stack>
    </ConfirmationDialog>
  );

  const createArchiveBtn = (
    <Button onClick={open} disabled={disconnectInProgress || !!osStatus.debugBundleStatus.inProgress} fullWidth={IS_HANDHELD_DEVICE}>
      {t('createDebugArchive')}
    </Button>
  );
  const loadingSpinner = !!osStatus.debugBundleStatus.inProgress &&
    <Group gap='sm' justify='center'><Text>{t('createDebugArchiveInProgress')}</Text><Loader size={ICON_SIZE} /></Group>;
  const archiveAvailable = !osStatus.debugBundleStatus.inProgress && osStatus.debugBundleStatus.latestPath !== null;
  if (IS_HANDHELD_DEVICE) {
    return (
      <>
        {modal}
        <Card withBorder radius='lg' p='lg' className={classes.card}>
          <Stack gap='md' align="center">
            <Title order={4} className={classes.havingTroubleTitle}>
              {t('havingTrouble')}
            </Title>
            <Text c='gray' component='span' ta='center'>
              <Trans i18nKey='supportMsgMobile' values={{ email: EMAIL }} components={[<Anchor href={mailto} />]} />
            </Text>
            {createArchiveBtn}
            {loadingSpinner}
            {archiveAvailable &&
              <>
                <Stack gap='sm' w='100%'>
                  <Button variant='light' onClick={() => shareDebugArchive(osStatus.debugBundleStatus.latestPath!)} data-disabled={!!osStatus.debugBundleStatus.inProgress} leftSection={<IoIosShare size={ICON_SIZE} />}>
                    {t('shareLatestDebugArchive')}
                  </Button>
                  <Button variant='light' onClick={() => emailDebugArchive(osStatus.debugBundleStatus.latestPath!, t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }), t('emailBodyIntro'))} disabled={!!osStatus.debugBundleStatus.inProgress || !osStatus.canSendMail} leftSection={<IoIosMail size={ICON_SIZE} />}>
                    {t('emailLatestDebugArchive')}
                  </Button>
                </Stack>
                {!osStatus.canSendMail && <Text c='red.7' fw={500} size='sm' ta='center'>{t('emailServiceUnavailable')}</Text>}
              </>}
          </Stack>
        </Card>
      </>
    );
  } else {
    return (
      <>
        {modal}
        <Group>
          {createArchiveBtn}
          {loadingSpinner}
          {archiveAvailable &&
            <Button variant='light' onClick={() => revealItemInDir(osStatus.debugBundleStatus.latestPath!)} disabled={!!osStatus.debugBundleStatus.inProgress}>
              {t('viewLatestDebugArchive')}
            </Button>}
        </Group>
      </>
    );
  }
}
