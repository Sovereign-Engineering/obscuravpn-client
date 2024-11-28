import { Alert, Button, Container, Group, Loader, Text } from '@mantine/core';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { useDebuggingArchive } from '../common/debuggingArchiveHook';

export default function DebuggingArchive() {
    const { t } = useTranslation();
    const [archiveState, createDebuggingArchive] = useDebuggingArchive();

    return (
        <>
            <Group grow>
                <Button onClick={createDebuggingArchive} disabled={archiveState.inProgress}>{t('createDebugArchive')}</Button>
                {archiveState.inProgress && <><Text>{t('createDebugArchiveInProgress')}</Text><Loader size={20} /></>}
            </Group>
            {archiveState?.error !== undefined && <>
                <Container w='100%'>
                    <Alert color='red' title={t('Error')}>{archiveState.error.message}<br />{t('Please get in touch')}</Alert>
                </Container>
            </>}
        </>
    );
}
