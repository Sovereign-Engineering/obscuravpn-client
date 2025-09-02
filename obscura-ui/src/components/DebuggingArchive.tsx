import { Button, Group, Loader, Stack, Text } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import { IoIosMail, IoIosShare } from 'react-icons/io';
import { revealItemInDir, emailArchive, shareFile } from '../bridge/commands';
import { IS_HANDHELD_DEVICE, systemName } from '../bridge/SystemProvider';
import { OsStatus } from '../common/appContext';
import { useDebuggingArchive } from '../common/debuggingArchiveHook';

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function DebuggingArchive({ osStatus }: { osStatus: OsStatus }) {
    const { t } = useTranslation();
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
            <Stack gap='sm'>
                {createArchiveBtn}
                {loadingSpinner}
                {archiveAvailable &&
                    <>
                        <Group gap='sm' grow>
                            <Button variant='light' onClick={() => emailArchive(osStatus.debugBundleStatus.latestPath!, t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }), t('emailBodyIntro'))} disabled={!!osStatus.debugBundleStatus.inProgress || !osStatus.canSendMail} leftSection={<IoIosMail size={iconSize} />}>
                                {t('emailLatestDebugArchive')}
                            </Button>
                            <Button variant='light' onClick={() => shareFile(osStatus.debugBundleStatus.latestPath!)} data-disabled={!!osStatus.debugBundleStatus.inProgress} leftSection={<IoIosShare size={iconSize} />}>
                                {t('shareLatestDebugArchive')}
                            </Button>
                        </Group>
                        {!osStatus.canSendMail && <Text c='red.7' fw={500} size='sm' ta='center'>{t('emailServiceUnavailable')}</Text>}
                    </>}
            </Stack>
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
