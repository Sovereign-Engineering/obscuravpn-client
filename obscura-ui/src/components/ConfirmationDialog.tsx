import { Drawer, DrawerProps, MantineSize, Modal } from '@mantine/core';
import { PropsWithChildren } from 'react';
import { useTranslation } from 'react-i18next';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import classes from './ConfirmationDialog.module.css';

interface ConfirmationDialogProps extends PropsWithChildren {
  opened: boolean;
  onClose: () => void;
  drawerSize?: MantineSize | (string & {}) | number;
  title?: string;
  drawerCloseButton?: boolean;
}

export function ConfirmationDialog({ opened, onClose, drawerSize = 'xs', title, children, drawerCloseButton }: ConfirmationDialogProps) {
  const { t } = useTranslation();
  return (
    IS_HANDHELD_DEVICE ?
      <MobileDrawer size={drawerSize} opened={opened} onClose={onClose} title={title ?? t('Confirmation')} withCloseButton={drawerCloseButton}>
        {children}
      </MobileDrawer> :
      <Modal opened={opened} onClose={onClose} title={title ?? t('Confirmation')} centered>
        {children}
      </Modal>
  );
}

type MobileDrawerProps = Omit<DrawerProps, 'classNames' | 'styles' | 'position'>;

export function MobileDrawer({ size, title, opened, onClose, children, withCloseButton }: MobileDrawerProps) {
  return (
    <Drawer classNames={{ content: classes.drawerContent, body: classes.drawerBody }} size={size} position='bottom' opened={opened} onClose={onClose} title={title} withCloseButton={withCloseButton}>
      {children}
    </Drawer>
  );
}
