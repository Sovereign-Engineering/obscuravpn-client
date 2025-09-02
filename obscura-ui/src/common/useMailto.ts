import { useTranslation } from 'react-i18next';
import { OsStatus } from './appContext';
import { EMAIL } from './links';
import { percentEncodeQuery } from './utils';
import { systemName } from '../bridge/SystemProvider';

// this component may be used before appContext is created, and thus requires explicitly passing osStatus
export default function useMailto(osStatus: OsStatus) {
  const { t } = useTranslation();

  // \r is important to ensure email clients do not trim newlines
  const params = {
    subject: t('emailSubject', { platform: systemName(), version: osStatus.srcVersion }),
    body: t('emailBodyIntro') + ':\n\n\r'
  };
  const queryString = percentEncodeQuery(params);
  const mailto = `mailto:${EMAIL}?${queryString}`;
  return mailto
}
