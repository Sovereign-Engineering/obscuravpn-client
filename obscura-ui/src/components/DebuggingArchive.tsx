import { Anchor, Button, Card, Group, Loader, Stack, Text, Title } from '@mantine/core';
import { Trans, useTranslation } from 'react-i18next';
import { IoIosMail, IoIosShare } from 'react-icons/io';
import { emailDebugArchive, revealItemInDir, shareDebugArchive } from '../bridge/commands';
import { IS_HANDHELD_DEVICE, systemName } from '../bridge/SystemProvider';
import { OsStatus } from '../common/appContext';
import { useDebuggingArchive } from '../common/debuggingArchiveHook';
import { EMAIL } from '../common/links';
import useMailto from '../common/useMailto';
import classes from './DebuggingArchive.module.css';

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function DebuggingArchive({ osStatus }: { osStatus: OsStatus }) {
    const { t } = useTranslation();
    const mailto = useMailto(osStatus);
    const createDebuggingArchive = useDebuggingArchive();
    const createArchiveBtn = (
        <Button onClick={createDebuggingArchive} disabled={!!osStatus.debugBundleStatus.inProgress} fullWidth={IS_HANDHELD_DEVICE}>
            {t('createDebugArchive')}
        </Button>
    );
    const iconSize = 20;
    const loadingSpinner = !!osStatus.debugBundleStatus.inProgress &&
        <Group gap='sm' justify='center'><Text>{t('createDebugArchiveInProgress')}</Text><Loader size={iconSize} /></Group>;
    const archiveAvailable = !osStatus.debugBundleStatus.inProgress && osStatus.debugBundleStatus.latestPath !== null;
    if (IS_HANDHELD_DEVICE) {
      return (
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
                  <Button variant='light' onClick={() => shareDebugArchive(osStatus.debugBundleStatus.latestPath!)} data-disabled={!!osStatus.debugBundleStatus.inProgress} leftSection={<IoIosShare size={iconSize} />}>
                    {t('shareLatestDebugArchive')}
                  </Button>
                  <Button variant='light' onClick={() => emailDebugArchive(osStatus.debugBundleStatus.latestPath!, t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }), t('emailBodyIntro'))} disabled={!!osStatus.debugBundleStatus.inProgress || !osStatus.canSendMail} leftSection={<IoIosMail size={iconSize} />}>
                    {t('emailLatestDebugArchive')}
                  </Button>
                </Stack>
                {!osStatus.canSendMail && <Text c='red.7' fw={500} size='sm' ta='center'>{t('emailServiceUnavailable')}</Text>}
              </>}
          </Stack>
        </Card>
        );
    } else {
        return (
            <Group>
                {createArchiveBtn}
                {loadingSpinner}
                {archiveAvailable &&
                    <Button variant='light' onClick={() => revealItemInDir(osStatus.debugBundleStatus.latestPath!)} disabled={!!osStatus.debugBundleStatus.inProgress}>
                        {t('viewLatestDebugArchive')}
                    </Button>}
            </Group>
        );
    }
}
