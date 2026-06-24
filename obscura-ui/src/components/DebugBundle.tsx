import { Anchor, Button, Card, Group, Loader, Stack, Text, Textarea, Title } from '@mantine/core';
import { useDisclosure } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { IoIosMail, IoIosShare } from 'react-icons/io';
import * as commands from '../bridge/commands';
import { emailDebugBundle, revealItemInDir, shareDebugBundle } from '../bridge/commands';
import { IS_HANDHELD_DEVICE, systemName } from '../bridge/SystemProvider';
import { NEVPNStatus, OsStatus, OsStatusWVpnStatus } from '../common/appContext';
import { useDebugBundle } from '../common/debugBundleHook';
import { EMAIL } from '../common/links';
import useMailto from '../common/useMailto';
import { normalizeError } from '../common/utils';
import { ErrorI18n, fmtErrorI18n } from '../translations/i18n';
import { ConfirmationDialog } from './ConfirmationDialog';
import classes from './DebugBundle.module.css';

const ICON_SIZE = 20;

export enum DebugBundleVariant {
  Card = 'card',
  LoginLabel = 'label'
}

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
// feedback may be controlled by a parent (e.g. HelpView, so its support link can include it); otherwise DebugBundle manages it internally
export default function DebugBundle({ osStatus, variant = DebugBundleVariant.Card, feedback, onFeedbackChange }: { osStatus: OsStatusWVpnStatus, variant?: DebugBundleVariant, feedback?: string, onFeedbackChange?: (value: string) => void }) {
  const { t } = useTranslation();
  const createDebugBundle = useDebugBundle();
  const [opened, { open: openModal, close }] = useDisclosure(false);
  const open = () => {
    setUserFeedback('');
    openModal();
  };
  const { execute: disconnect } = commands.useCommand({ command: commands.disconnect, showNotification: false, rethrow: true });
  const [disconnectInProgress, setDisableButtons] = useState(false);
  const [internalFeedback, setInternalFeedback] = useState('');
  const userFeedback = feedback !== undefined ? feedback : internalFeedback;
  const setUserFeedback = onFeedbackChange ?? setInternalFeedback;

  const onContinue = () => {
    setDisableButtons(false);
    void createDebugBundle(userFeedback);
    if (variant !== DebugBundleVariant.LoginLabel) {
      close();
    }
  }

  const loadingSpinner = !!osStatus.debugBundleStatus.inProgress &&
    <Group gap='sm' justify='center'><Text>{t('createDebugBundleInProgress')}</Text><Loader size={ICON_SIZE} /></Group>;
  const archiveAvailable = !osStatus.debugBundleStatus.inProgress && osStatus.debugBundleStatus.latestPath !== null;
  const showStatus = osStatus.debugBundleStatus.inProgress || osStatus.debugBundleStatus.latestPath !== null;

  const modal = (
    <ConfirmationDialog title={t('Debug Bundle')} opened={opened} onClose={close}>
      <Stack h='100%' justify='space-between' gap='xs'>
        {
          osStatus.osVpnStatus !== NEVPNStatus.Disconnected
          && <>
            <Text>{t('debugBundleDisconnectPrompt')}</Text>
          </>
        }
        <Textarea
          data-autofocus
          label={t('debugBundleFeedbackLabel')}
          placeholder={t('debugBundleFeedbackPrompt')}
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
              try {
                await disconnect();
                await commands.waitUntilDisconnected(osStatus);
                onContinue();
              } catch (err) {
                console.error('failed to disconnect before creating debug bundle');
                const error = normalizeError(err);
                const message = error instanceof ErrorI18n
                  ? fmtErrorI18n(t, error)
                  : error.message;
                notifications.show({
                  color: 'red',
                  title: t('Error'),
                  message
                });
                setDisableButtons(false);
              }
            }}>{disconnectInProgress ? <Loader size={ICON_SIZE} /> : t('Disconnect')}</Button>
          }
        </Group>
        {variant === DebugBundleVariant.LoginLabel && showStatus && (
          <>
            {loadingSpinner ||
              <Stack gap='sm'>
                <SupportMessage osStatus={osStatus} size='sm' color='dimmed' userFeedback={userFeedback} />
                <ArchiveActionButtons osStatus={osStatus} userFeedback={userFeedback} />
              </Stack>
            }
          </>
        )}
      </Stack>
    </ConfirmationDialog>
  );

  if (variant === DebugBundleVariant.LoginLabel) {
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
      {t('createDebugBundle')}
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
            <SupportMessage osStatus={osStatus} color='gray' userFeedback={userFeedback} />
            {createArchiveBtn}
            {loadingSpinner}
            {archiveAvailable && <Stack gap='sm' w='100%'><ArchiveActionButtons osStatus={osStatus} inProgress={!!osStatus.debugBundleStatus.inProgress} userFeedback={userFeedback} /></Stack>}
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
          {archiveAvailable && <ArchiveActionButtons osStatus={osStatus} inProgress={!!osStatus.debugBundleStatus.inProgress} userFeedback={userFeedback} />}
        </Group>
      </>
    );
  }
}

interface SupportMessageProps {
  osStatus: OsStatus;
  size?: 'sm';
  color?: string;
  userFeedback?: string;
}

function SupportMessage({ osStatus, size, color, userFeedback }: SupportMessageProps) {
  const mailto = useMailto(osStatus, userFeedback);
  return (
    <Text size={size} c={color} ta='center' component={size ? undefined : 'span'}>
      <Trans i18nKey='supportMsgOrDebugBundle' values={{ email: EMAIL }} components={[<Anchor href={mailto} />]} />
    </Text>
  );
};

interface ArchiveActionButtonsProps {
  osStatus: OsStatus;
  inProgress?: boolean;
  userFeedback?: string;
}

function ArchiveActionButtons({ osStatus, inProgress = false, userFeedback = '' }: ArchiveActionButtonsProps) {
  const { t } = useTranslation();

  if (IS_HANDHELD_DEVICE) {
    return (
      <>
        <Button variant='light' onClick={() => shareDebugBundle(osStatus.debugBundleStatus.latestPath!)} data-disabled={inProgress} leftSection={<IoIosShare size={ICON_SIZE} />}>
          {t('shareLatestDebugBundle')}
        </Button>
        <Button variant='light' onClick={() => emailDebugBundle(osStatus.debugBundleStatus.latestPath!, t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }), userFeedback ? t('emailBodyIntro') + ':\n\n' + userFeedback : t('emailBodyIntro'))} disabled={inProgress || !osStatus.canSendMail} leftSection={<IoIosMail size={ICON_SIZE} />}>
          {t('emailLatestDebugBundle')}
        </Button>
        {!osStatus.canSendMail && <Text c='red.7' fw={500} size='sm' ta='center'>{t('emailServiceUnavailable')}</Text>}
      </>
    );
  } else {
    return (
      <Button variant='light' onClick={() => revealItemInDir(osStatus.debugBundleStatus.latestPath!)} disabled={inProgress}>
        {t('viewLatestDebugBundle')}
      </Button>
    );
  }
};
