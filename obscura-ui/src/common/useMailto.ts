import { useTranslation } from 'react-i18next';
import { OsStatus } from './appContext';
import { EMAIL } from './links';
import { percentEncodeQuery } from './utils';
import { systemName } from '../bridge/SystemProvider';

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function useMailto(osStatus: OsStatus, userFeedback?: string) {
  const { t } = useTranslation();

  // \r is important to ensure email clients do not trim newlines
  const body = userFeedback
    ? t('emailBodyIntro') + ':\n\n' + userFeedback
    : t('emailBodyIntro') + ':\n\n\r';
  const params = {
    subject: t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }),
    body
  };
  const queryString = percentEncodeQuery(params);
  const mailto = `mailto:${EMAIL}?${queryString}`;
  return mailto
}
