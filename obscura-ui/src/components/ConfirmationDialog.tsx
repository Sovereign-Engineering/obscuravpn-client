import { Drawer, DrawerProps, MantineSize, Modal } from '@mantine/core';
import { PropsWithChildren } from 'react';
import { useTranslation } from 'react-i18next';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import commonClasses from '../common/common.module.css';
import classes from './ConfirmationDialog.module.css';

interface ConfirmationDialogProps extends PropsWithChildren {
  opened: boolean;
  onClose: () => void;
  drawerSize?: MantineSize | (string & {}) | number;
  title?: string;
  drawerCloseButton?: boolean;
  closeOnClickOutside?: boolean;
  closeOnEscape?: boolean;
  withCloseButton?: boolean;
  contentClassName?: string;
  bodyClassName?: string;
  innerClassName?: string;
}

export function ConfirmationDialog({ opened, onClose, drawerSize = 'xs', title, children, drawerCloseButton, closeOnClickOutside, closeOnEscape, withCloseButton, contentClassName, bodyClassName, innerClassName }: ConfirmationDialogProps) {
  const { t } = useTranslation();
  return (
    IS_HANDHELD_DEVICE ?
      <MobileDrawer
        size={drawerSize}
        opened={opened}
        onClose={onClose}
        title={title ?? t('Confirmation')}
        withCloseButton={withCloseButton ?? drawerCloseButton}
        closeOnClickOutside={closeOnClickOutside}
        closeOnEscape={closeOnEscape}
        contentClassName={contentClassName}
        bodyClassName={bodyClassName}
      >
        {children}
      </MobileDrawer> :
      <Modal
        opened={opened}
        onClose={onClose}
        title={title ?? t('Confirmation')}
        centered
        withCloseButton={withCloseButton}
        closeOnClickOutside={closeOnClickOutside}
        closeOnEscape={closeOnEscape}
        classNames={{ content: contentClassName, body: bodyClassName, inner: innerClassName }}
      >
        {children}
      </Modal>
  );
}

type MobileDrawerProps = Omit<DrawerProps, 'classNames' | 'styles' | 'position'> & {
  contentClassName?: string;
  bodyClassName?: string;
};

export function MobileDrawer({ size, title, opened, onClose, children, withCloseButton, contentClassName, bodyClassName, ...others }: MobileDrawerProps) {
  const contentClass = `${classes.drawerContent} ${commonClasses.applySideInsets}${contentClassName ? ` ${contentClassName}` : ''}`;
  const bodyClass = `${classes.drawerBody}${bodyClassName ? ` ${bodyClassName}` : ''}`;
  return (
    <Drawer classNames={{ content: contentClass, body: bodyClass }} size={size} position='bottom' opened={opened} onClose={onClose} title={title} withCloseButton={withCloseButton} {...others}>
      {children}
    </Drawer>
  );
}
