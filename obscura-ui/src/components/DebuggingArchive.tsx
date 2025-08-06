import { Button, Group, Loader, Text } from '@mantine/core';
import { useTranslation } from 'react-i18next';
import { revealItemInDir } from '../bridge/commands';
import { OsStatus } from '../common/appContext';
import { useDebuggingArchive } from '../common/debuggingArchiveHook';

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function DebuggingArchive({ osStatus }: { osStatus: OsStatus}) {
    const { t } = useTranslation();
    const createDebuggingArchive = useDebuggingArchive();

    return (
        <>
            <Group>
                <Button onClick={createDebuggingArchive} disabled={!!osStatus.debugBundleStatus.inProgress}>{t('createDebugArchive')}</Button>
                {!!osStatus.debugBundleStatus.inProgress &&
                    <><Text>{t('createDebugArchiveInProgress')}</Text><Loader size={20} /></>}
                {!osStatus.debugBundleStatus.inProgress && osStatus.debugBundleStatus.latestPath !== null &&
                    <Button variant='light' onClick={() => revealItemInDir(osStatus.debugBundleStatus.latestPath!)} disabled={!!osStatus.debugBundleStatus.inProgress}>
                        {t('viewLatestDebugArchive')}
                    </Button>}
            </Group>
        </>
    );
}
