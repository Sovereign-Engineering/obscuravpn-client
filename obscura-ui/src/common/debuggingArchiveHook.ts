import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { debuggingArchive } from '../tauri/commands';

type ArchiveState = { inProgress: boolean, error?: Error };

export function useDebuggingArchive(): [ArchiveState, () => Promise<void>] {
    const { t } = useTranslation();
    const [archiveState, setArchiveState] = useState<ArchiveState>({ inProgress: false });

    const startCreatingArchive = async () => {
        setArchiveState({ inProgress: true });
        try {
            await debuggingArchive();
            notifications.show({
                title: t('Debugging Archive Created'),
                message: t('findDebugBundleInFinder')
            });
            setArchiveState({ inProgress: false });
        } catch (error) {
            if (typeof error === 'string') {
                error = new Error(error);
            }
            setArchiveState({ inProgress: false, error });
        }
    }
    return [archiveState, startCreatingArchive];
}
