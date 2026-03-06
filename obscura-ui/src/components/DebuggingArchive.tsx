import { Anchor, Button, Card, Group, Loader, Stack, Text, Textarea, Title } from '@mantine/core';
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

export enum DebuggingArchiveVariant {
  Card = 'card',
  LoginLabel = 'label'
}

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function DebuggingArchive({ osStatus, variant = DebuggingArchiveVariant.Card }: { osStatus: OsStatus, variant?: DebuggingArchiveVariant }) {
  const { t } = useTranslation();
  const createDebuggingArchive = useDebuggingArchive();
  const [opened, { open, close }] = useDisclosure(false);
  const { execute: disconnect } = commands.useCommand({ command: commands.disconnect, showNotification: true, rethrow: true });
  const [disconnectInProgress, setDisableButtons] = useState(false);
  const [userFeedback, setUserFeedback] = useState('');

  const onContinue = () => {
    setDisableButtons(false);
    void createDebuggingArchive(userFeedback);
    // For Label variant, keep modal open to show status
    if (variant !== DebuggingArchiveVariant.LoginLabel) {
      close();
      setUserFeedback('');
    }
  }

  const loadingSpinner = !!osStatus.debugBundleStatus.inProgress &&
    <Group gap='sm' justify='center'><Text>{t('createDebugArchiveInProgress')}</Text><Loader size={ICON_SIZE} /></Group>;
  const archiveAvailable = !osStatus.debugBundleStatus.inProgress && osStatus.debugBundleStatus.latestPath !== null;
  const showStatus = osStatus.debugBundleStatus.inProgress || osStatus.debugBundleStatus.latestPath !== null;

  const modal = (
    <ConfirmationDialog title={t('Debugging Archive')} opened={opened} onClose={close}>
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
          <Button disabled={disconnectInProgress || !!loadingSpinner} miw={130} onClick={onContinue} variant='light'>{
            osStatus.osVpnStatus === NEVPNStatus.Disconnected ?
              t('Continue') : t('Stay Connected')
          }</Button>
          {
            osStatus.osVpnStatus !== NEVPNStatus.Disconnected &&
            <Button disabled={disconnectInProgress} miw={130} onClick={async () => {
              setDisableButtons(true);
              await disconnect();
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
        {variant === DebuggingArchiveVariant.LoginLabel && showStatus && (
          <>
            {loadingSpinner ||
              <Stack gap='sm'>
                <SupportMessage osStatus={osStatus} size='sm' color='dimmed' />
                <ArchiveActionButtons osStatus={osStatus} />
              </Stack>
            }
          </>
        )}
      </Stack>
    </ConfirmationDialog>
  );

  if (variant === DebuggingArchiveVariant.LoginLabel) {
    /**
     * on hand held, the decoration is always at the bottom, even in landscape
     * we want the label just above the decoration in portrait, and at the top in landscape
     * When the keyboard is shown, there isn't enough space for a label, so use a help icon instead
     */
    if (IS_HANDHELD_DEVICE) {
      return (
        <>
          {modal}
          <Text className={`${classes.debugLabel} ${classes.debugLabelHandheld}`} p='xs' size='xs' c='dimmed'>
              <Trans i18nKey='experiencingIssues' components={[<wbr />, <Anchor component='button' type='button' c='orange' onClick={open} style={{ cursor: 'pointer' }} />]} />
          </Text>
        </>
      );
    }

    return (
      <>
        {modal}
        <Text className={classes.debugLabel} p='xs' size='xs' c='dimmed'>
          <Trans i18nKey='experiencingIssues' components={[<wbr />, <Anchor component='button' type='button' c='orange' onClick={open} style={{ cursor: 'pointer' }} />]} />
        </Text>
      </>
    );
  }

  const createArchiveBtn = (
    <Button onClick={open} disabled={disconnectInProgress || !!osStatus.debugBundleStatus.inProgress} fullWidth={IS_HANDHELD_DEVICE}>
      {t('createDebugArchive')}
    </Button>
  );
  if (IS_HANDHELD_DEVICE) {
    return (
      <>
        {modal}
        <Card withBorder radius='lg' p='lg' className={classes.card}>
          <Stack gap='md' align='center'>
            <Title order={4} className={classes.havingTroubleTitle}>
              {t('havingTrouble')}
            </Title>
            <SupportMessage osStatus={osStatus} color='gray' />
            {createArchiveBtn}
            {loadingSpinner}
            {archiveAvailable && <Stack gap='sm' w='100%'><ArchiveActionButtons osStatus={osStatus} inProgress={!!osStatus.debugBundleStatus.inProgress} /></Stack>}
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
          {archiveAvailable && <ArchiveActionButtons osStatus={osStatus} inProgress={!!osStatus.debugBundleStatus.inProgress} />}
        </Group>
      </>
    );
  }
}

interface SupportMessageProps {
  osStatus: OsStatus;
  size?: 'sm';
  color?: string;
}

function SupportMessage({ osStatus, size, color }: SupportMessageProps) {
  const mailto = useMailto(osStatus);
  return (
    <Text size={size} c={color} ta='center' component={size ? undefined : 'span'}>
      <Trans i18nKey='supportMsgOrDebugArchive' values={{ email: EMAIL }} components={[<Anchor href={mailto} />]} />
    </Text>
  );
};

interface ArchiveActionButtonsProps {
  osStatus: OsStatus;
  inProgress?: boolean;
}

function ArchiveActionButtons({ osStatus, inProgress = false }: ArchiveActionButtonsProps) {
  const { t } = useTranslation();

  if (IS_HANDHELD_DEVICE) {
    return (
      <>
        <Button variant='light' onClick={() => shareDebugArchive(osStatus.debugBundleStatus.latestPath!)} data-disabled={inProgress} leftSection={<IoIosShare size={ICON_SIZE} />}>
          {t('shareLatestDebugArchive')}
        </Button>
        <Button variant='light' onClick={() => emailDebugArchive(osStatus.debugBundleStatus.latestPath!, t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }), t('emailBodyIntro'))} disabled={inProgress || !osStatus.canSendMail} leftSection={<IoIosMail size={ICON_SIZE} />}>
          {t('emailLatestDebugArchive')}
        </Button>
        {!osStatus.canSendMail && <Text c='red.7' fw={500} size='sm' ta='center'>{t('emailServiceUnavailable')}</Text>}
      </>
    );
  } else {
    return (
      <Button variant='light' onClick={() => revealItemInDir(osStatus.debugBundleStatus.latestPath!)} disabled={inProgress}>
        {t('viewLatestDebugArchive')}
      </Button>
    );
  }
};
