import { ActionIcon, Button } from '@mantine/core';
import { ReactNode, useContext, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { MdArrowForward, MdClose } from 'react-icons/md';
import { accountIdToString } from '../common/accountUtils';
import { AppContext } from '../common/appContext';
import { LOCK_IN_CREDIT } from '../common/links';
import { useCookie } from '../common/utils';
import DecoAccent from '../res/deco/deco-bit-accent.svg?react';
import JailDeco from '../res/lockin/jail-deco.svg?react';
import Wordmark from '../res/lockin/wordmark.svg?react';
import { ConfirmationDialog } from './ConfirmationDialog';
import classes from './LockInPromo.module.css';

const LOCK_IN_PROMO_SEEN = 'lock-in-promo-seen';
const LOCK_IN_BANNER_SEEN = 'lock-in-banner-seen';

function parseAccountIds(value: string): string[] {
  return value.split(',').filter(Boolean);
}

export function LockInPromo() {
  const { accountInfo } = useContext(AppContext);
  const [opened, setOpened] = useState(false);
  const [seenAccountIds, setSeenAccountIds] = useCookie(LOCK_IN_PROMO_SEEN, '');
  const [bannerSeenAccountIds, setBannerSeenAccountIds] = useCookie(LOCK_IN_BANNER_SEEN, '');

  const accountId = accountInfo ? accountIdToString(accountInfo.id) : null;
  const eligible = accountInfo?.nnn != null;
  const popupSeen = accountId !== null && parseAccountIds(seenAccountIds).includes(accountId);
  const bannerSeen = accountId !== null && parseAccountIds(bannerSeenAccountIds).includes(accountId);

  useEffect(() => {
    if (eligible && accountId !== null && !popupSeen) {
      setOpened(true);
    }
  }, [eligible, accountId, popupSeen]);

  const closePopup = () => {
    setOpened(false);
    if (accountId === null || popupSeen) {
      return;
    }
    setSeenAccountIds([...parseAccountIds(seenAccountIds), accountId].join(','));
  };

  const closeBanner = () => {
    if (accountId === null || bannerSeen) {
      return;
    }
    setBannerSeenAccountIds([...parseAccountIds(bannerSeenAccountIds), accountId].join(','));
  };

  if (!eligible) {
    return null;
  }

  return (
    <>
      {!bannerSeen && <Banner onClose={closeBanner} />}
      <ConfirmationDialog
        opened={opened}
        onClose={closePopup}
        title=''
        withCloseButton={false}
        drawerSize='lg'
        contentClassName={classes.modalContent}
        bodyClassName={classes.modalBody}
        innerClassName={classes.modalInner}
      >
        <Popup onClose={closePopup} />
      </ConfirmationDialog>
    </>
  );
}

function Banner({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <section className={classes.banner}>
      <JailDeco className={classes.bannerDeco} aria-hidden />
      <div className={classes.bannerText}>
        <span className={classes.bannerTitle}>{t('lockInBannerTitle')}</span>
        <span className={classes.bannerSubtitle}>
          <span className={classes.compactCopy}>{t('lockInBannerSubtitleShort')}</span>
          <span className={classes.fullCopy}>{t('lockInBannerSubtitle')}</span>
        </span>
      </div>
      <PromoCta href={LOCK_IN_CREDIT}>{t('lockInCtaShort')}</PromoCta>
      <CloseButton className={classes.bannerClose} onClick={onClose} />
    </section>
  );
}

function CloseButton({ className, onClick }: { className?: string; onClick: () => void }) {
  return (
    <ActionIcon
      className={className}
      variant='transparent'
      c='white'
      onClick={onClick}
    >
      <MdClose size={24} />
    </ActionIcon>
  );
}

function Popup({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  return (
    <div className={classes.popup}>
      <div className={classes.accentClip} aria-hidden>
        <DecoAccent className={classes.accentLeft} />
        <DecoAccent className={classes.accentRight} />
      </div>
      <CloseButton className={classes.close} onClick={onClose} />
      <JailDeco className={classes.popupDeco} aria-hidden />
      <div className={classes.popupContent}>
        <Wordmark className={classes.wordmark} aria-hidden />
        <div className={classes.text}>
          <h2 className={classes.title}>{t('lockInPopupTitle')}</h2>
          <span className={classes.subtitle}>{t('lockInPopupSubtitle')}</span>
        </div>
        <PromoCta href={LOCK_IN_CREDIT}>{t('lockInCta')}</PromoCta>
      </div>
    </div>
  );
}

interface PromoCtaProps {
  onClick?: () => void;
  href?: string;
  children: ReactNode;
}

function PromoCta({ onClick, href, children }: PromoCtaProps) {
  return (
    <Button
      className={classes.cta}
      variant='default'
      component={href ? 'a' : 'button'}
      href={href}
      target={href ? '_blank' : undefined}
      rightSection={<MdArrowForward size={16} />}
      onClick={onClick}
    >
      {children}
    </Button>
  );
}
