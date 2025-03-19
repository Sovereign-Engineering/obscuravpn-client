import { Button, Group, Loader, Text } from '@mantine/core';
import { useContext } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { revealItemInDir } from '../bridge/commands';
import { AppContext } from '../common/appContext';
import { useDebuggingArchive } from '../common/debuggingArchiveHook';

export default function DebuggingArchive() {
    const { t } = useTranslation();
    const { osStatus } = useContext(AppContext);
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
