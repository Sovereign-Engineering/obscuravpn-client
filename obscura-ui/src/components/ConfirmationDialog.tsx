import { Drawer, MantineSize, Modal } from '@mantine/core';
import { PropsWithChildren } from 'react';
import { useTranslation } from 'react-i18next';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import commonClasses from '../common/common.module.css';

interface ConfirmationDialogProps extends PropsWithChildren {
  opened: boolean;
  onClose: () => void;
  drawerSize?: MantineSize | (string & {}) | number;
  title?: string;
}

export function ConfirmationDialog({ opened, onClose, drawerSize = 'xs', title, children }: ConfirmationDialogProps) {
  const { t } = useTranslation();
  return (
    IS_HANDHELD_DEVICE ?
      <Drawer classNames={{ content: commonClasses.bottomSheet }} size={drawerSize} position='bottom' opened={opened} onClose={onClose} title={title ?? t('Confirmation')}>
        {children}
      </Drawer> :
      <Modal opened={opened} onClose={onClose} title={title ?? t('Confirmation')} centered>
        {children}
      </Modal>
    );
}
