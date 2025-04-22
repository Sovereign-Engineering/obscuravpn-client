import { Anchor, Text } from '@mantine/core';
import { notifications } from '@mantine/notifications';
import { useState } from 'react';
import { Trans, useTranslation } from 'react-i18next';
import { CommandError, debuggingArchive, revealItemInDir } from '../bridge/commands';
import { fmtErrorI18n } from '../translations/i18n';
import { normalizeError } from './utils';

type ArchiveState = { inProgress: boolean, error?: Error };

export function useDebuggingArchive(): () => Promise<void> {
    const { t } = useTranslation();
    const [_, setArchiveState] = useState<ArchiveState>({ inProgress: false });

    const startCreatingArchive = async () => {
        setArchiveState({ inProgress: true });
        try {
            const path = await debuggingArchive();
            notifications.show({
                title: t('Debugging Archive Created'),
                message: <Text><Trans i18nKey='findDebugBundleInFinder' components={[<Anchor onClick={() => revealItemInDir(path)} />]} /></Text >
            });
        } catch (e) {
          const error = normalizeError(e);
          const message = error instanceof CommandError
              ? fmtErrorI18n(t, error) : error.message;
          notifications.show({
              title: t('Debugging Archive Failed'),
              message,
              color: 'red'
          });
        }
    }
    return startCreatingArchive;
}
