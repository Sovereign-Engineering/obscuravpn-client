import { AppShell, AppShellMain, Modal, Text, Title, useMantineColorScheme } from '@mantine/core';
import { useHotkeys, useThrottledValue } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { Trans, useTranslation } from 'react-i18next';
import { Navigate, Route, Routes, useNavigate } from 'react-router-dom';
import SimpleBar from 'simplebar-react';
import 'simplebar-react/dist/simplebar.min.css';
import classes from './App.module.css';
import * as commands from './bridge/commands';
import { IS_HANDHELD_DEVICE, logReactError, PLATFORM, Platform, useSystemChecks } from './bridge/SystemProvider';
import { AppContext, AppStatus, ConnectionInProgress, isConnectingStatus, NEVPNStatus, OsStatus } from './common/appContext';
import { fmt } from './common/fmt';
import { NotificationId } from './common/notifIds';
import { useAsync } from './common/useAsync';
import { useLoadable } from './common/useLoadable';
import { MIN_LOAD_MS, normalizeError } from './common/utils';
import { ScrollToTop } from './components/ScrollToTop';
import { fmtVpnError } from './translations/i18n';
import { About, Account, Connection, DeveloperView, FallbackAppRender, Help, Location, LogIn, Settings, SplashScreen } from './views';

// imported views need to be added to the `views` list variable
interface View {
  component: () => ReactNode,
  path: string,
  exact?: boolean,
  name: string
}

export default function () {
  const { t } = useTranslation();
  // Boilerplate State
  const navigate = useNavigate();
  const { toggleColorScheme } = useMantineColorScheme();
  useSystemChecks();
  useHotkeys([[PLATFORM === Platform.macOS ? 'mod+J' : 'ctrl+J', toggleColorScheme]]);
  const [scroller, setScroller] = useState<HTMLElement | null>(null);

  // App State
  const [vpnConnected, setVpnConnected] = useState(false);
  // keep track of how the connection was initiated to show correct transitioning UI
  const [initiatingExitSelector, setExitSelector] = useState<commands.ExitSelector>();
  const [connectionInProgress, setConnectionInProgress] = useState<ConnectionInProgress>(ConnectionInProgress.UNSET);
  const [warningNotices, setWarningNotices] = useState<string[]>([]);
  const [importantNotices, setImportantNotices] = useState<string[]>([]);
  const [appStatus, setStatus] = useState<AppStatus | null>(null);
  const [osStatus, setOsStatus] = useState<OsStatus | null>(null);
  const ignoreConnectingErrors = useRef(false);

  const views: View[] = [
    { component: Connection, path: '/connection', name: t('Connection') },
    { component: DeveloperView, path: '/developer', name: t('Developer') },
    { component: Location, path: '/location', name: t('Location') },
    { component: Account, path: '/account', name: t('Account') },
    { component: Help, path: '/help', name: t('Help') },
    { component: About, path: '/about', name: t('About') },
    { component: Settings, path: '/settings', name: t('Settings') },
  ];

  const isLoggedIn = !!appStatus?.accountId;
  const showAccountCreation = appStatus?.inNewAccountFlow;
  const loading = appStatus === null || osStatus === null;

  useEffect(() => {
    // reminder: errors are auto logged
    commands.notices().then(notices => {
      const warnNotices: string[] = [];
      const importantNotices: string[] = [];
      notices.forEach(notice => {
        const content = notice.content;
        switch (notice.type) {
          case 'Warn':
            warnNotices.push(content);
            break;
          case 'Important':
          // in case of a refactoring of the Error notice type
          case 'Error':
            importantNotices.push(content);
            break;
          default:
            console.error(`unhandled notice type ${notice.type}`);
        }
      });
      setWarningNotices(warnNotices);
      setImportantNotices(importantNotices);
    })
  }, []);

  async function tryConnect(exit: commands.ExitSelector) {
    setExitSelector(exit);
    if (vpnConnected) {
      setConnectionInProgress(ConnectionInProgress.ChangingLocations);
    } else {
      setConnectionInProgress(ConnectionInProgress.Connecting);
    }
    ignoreConnectingErrors.current = false;
    try {
      await commands.connect(exit);
    } catch (e) {
      const error = normalizeError(e);
      if (error.message === 'accountExpired') {
        void pollAccount();
      }
      if (!ignoreConnectingErrors.current && error.message !== 'tunnelNotDisconnected') {
        notifications.hide(NotificationId.VPN_ERROR);
        notifications.show({ title: t('Error Connecting'), message: fmtVpnError(t, error.message), color: 'red', id: NotificationId.VPN_ERROR, autoClose: false });
        // see https://linear.app/soveng/issue/OBS-775/not-starting-tunnel-because-it-isnt-disconnected-connecting#comment-e98a7150
        setConnectionInProgress(ConnectionInProgress.UNSET);
      }
    }
  }

  async function disconnectFromVpn() {
    ignoreConnectingErrors.current = true;
    setConnectionInProgress(ConnectionInProgress.Disconnecting);
    setVpnConnected(false);
    await commands.disconnect();
  }

  function notifyVpnError(errorEnum: string) {
    // see enum JsVpnError in commands.swift
    if (errorEnum !== null) {
      notifications.hide(NotificationId.VPN_ERROR);
      notifications.show({
        id: NotificationId.VPN_ERROR,
        withCloseButton: true,
        color: 'red',
        title: t('Error'),
        message: fmtVpnError(t, errorEnum),
        autoClose: 15_000
      });
    }
  }

  function handleNewStatus(newStatus: AppStatus) {
    const vpnStatus = newStatus.vpnStatus;
    if (vpnStatus === undefined) return;

    if (vpnStatus.connected !== undefined) {
      setVpnConnected(true);
      setConnectionInProgress(ConnectionInProgress.UNSET);
      notifications.hide(NotificationId.VPN_ERROR);
      notifications.update({
        id: NotificationId.VPN_DISCONNECT_CONNECT,
        message: undefined,
        color: 'green',
        autoClose: 1000
      });
    } else if (vpnStatus.connecting !== undefined) {
      setVpnConnected(false);
      const reconnecting = vpnStatus.connecting.reconnecting;
      setConnectionInProgress(value => {
        if (reconnecting) return ConnectionInProgress.Reconnecting;
        if (value === ConnectionInProgress.ChangingLocations) return value;
        return ConnectionInProgress.Connecting;
      });
      const connectError = vpnStatus.connecting?.connectError;
      if (connectError !== undefined) {
        if (reconnecting) {
          console.error(`got error while reconnecting: ${connectError}`);
        } else {
          console.error(`got error while connecting: ${connectError}`);
        }
        console.log(fmt`vpnStatus = ${vpnStatus}`);
        notifyVpnError(connectError);
      }
    }
  }

  // this code fetches the status of the VPN continuously
  // getting the status is blocking and takes an ID such that if non-null, only new statuses will be returned
  useEffect(() => {
    let knownStatusId = null;
    let keepAlive = true;
    (async () => {
      while (keepAlive) {
        try {
          let newStatus = await commands.status(knownStatusId);
          knownStatusId = newStatus.version;
          setStatus(newStatus);
        } catch (error) {
          const e = normalizeError(error);
          console.error('command status failed', e.message);
          notifications.show({ title: t('errorFetchingStatus'), message: e.message, color: 'red' });
        }
      }
    })();
    return () => { keepAlive = false; };
  }, []);

  useEffect(() => {
    let knownOsStatusId = null;
    let keepAlive = true;
    (async () => {
      while (keepAlive) {
        try {
          let newOsStatus = await commands.osStatus(knownOsStatusId);
          knownOsStatusId = newOsStatus.version;
          setOsStatus(newOsStatus);
        } catch (error) {
          const e = normalizeError(error);
          console.error('command osStatus failed', e.message);
          notifications.show({ title: t('errorFetchingOsStatus'), message: e.message, color: 'red' });
        }
      }
    })();
    return () => { keepAlive = false; };
  }, []);

  useEffect(() => {
    if (appStatus !== null) handleNewStatus(appStatus);
  }, [appStatus]);

  useEffect(() => {
    if (osStatus !== null) {
      const { osVpnStatus } = osStatus;
      if (osVpnStatus === NEVPNStatus.Disconnecting) {
        setConnectionInProgress(ConnectionInProgress.Disconnecting);
      } else if (osVpnStatus === NEVPNStatus.Disconnected) {
        setConnectionInProgress(ConnectionInProgress.UNSET);
        setVpnConnected(false);
        setExitSelector(undefined);
      }
    }
  }, [osStatus]);

  function resetState() {
    if (window.location.pathname === '/connection') {
      window.location.pathname = '/help';
    } else {
      window.location.pathname = '/';
    }
  }

  // native driven navigation
  useEffect(() => {
    const onNavUpdate = (e: Event) => {
      if (e instanceof CustomEvent) {
        navigate(`/${e.detail}`);
      } else {
        console.error('expected custom event for navigation purposes, got generic Event');
      }
    };
    window.addEventListener('navUpdate', onNavUpdate);
    return () => window.removeEventListener('navUpdate', onNavUpdate);
  }, []);

  const onPaymentSucceeded = () => {
    console.log("handling paymentSucceeded event");
    void pollAccount();
    commands.setInNewAccountFlow(false);
  }

  // deep link payment succeeded
  useEffect(() => {
    window.addEventListener('paymentSucceeded', onPaymentSucceeded);
    return () => window.removeEventListener('paymentSucceeded', onPaymentSucceeded);
  }, []);

  const {
    lastSuccessfulValue: accountInfo,
    error: accountInfoError,
    refresh: pollAccount,
    loading: accountLoading
  } = useLoadable({
    skip: !osStatus?.internetAvailable || !isLoggedIn,
    load: commands.getAccount,
    periodMs: showAccountCreation ? 60 * 1000 : 12 * 3600 * 1000,
    returnError: true,
  });
  const accountLoadingDelayed = useThrottledValue(accountLoading, accountLoading ? MIN_LOAD_MS : 0);

  useEffect(() => {
    if (accountInfoError) {
      console.error("Failed to fetch account info", accountInfoError);
      // We just ignore errors, they will be shown if the user goes to the account page.
    }
  }, [accountInfoError]);

  const _ = useAsync({
    skip: osStatus === null || (!osStatus.internetAvailable || IS_HANDHELD_DEVICE),
    load: commands.checkForUpdates,
    returnError: true,
  });

  if (loading) return <SplashScreen text={t('appStatusLoading')} />;

  if (!isLoggedIn || showAccountCreation) return <LogIn accountNumber={appStatus.accountId} accountActive={accountInfo?.active} />;

  const appContext = {
    accountInfo: accountInfo ?? null,
    appStatus,
    connectionInProgress,
    osStatus,
    pollAccount,
    isOffline: !osStatus.internetAvailable && !vpnConnected && !isConnectingStatus(connectionInProgress, osStatus.osVpnStatus),
    accountLoading: accountLoadingDelayed,
    vpnConnect: tryConnect,
    vpnConnected,
    vpnDisconnect: disconnectFromVpn,
    initiatingExitSelector,
  }

  // <> is an alias for <React.Fragment>
  return <>
    {/* non-closable notice */}
    <Modal size='100%' overlayProps={{ backgroundOpacity: 0.7 }} opened={importantNotices.length > 0} withCloseButton={false} onClose={() => { }}
      title={<Title order={5} style={{ color: 'orangered', letterSpacing: 1.5, textDecoration: 'bold' }}>{t('IMPORTANT NOTICE', { count: importantNotices.length })}</Title>}>
      {importantNotices.map(notice => <Text style={{ marginBottom: 10 }}><Trans i18nKey='importantNotice' values={{ notice, count: importantNotices.length }} /></Text>)}
    </Modal>
    <AppShell
      header={{ height: 0 }}
      navbar={undefined}
      className={classes.appShell}>
      <AppShellMain>
        <SimpleBar scrollableNodeProps={{ ref: setScroller }} autoHide={false} className={classes.simpleBar}>
          <AppContext.Provider value={appContext}>
            <ErrorBoundary FallbackComponent={FallbackAppRender} onReset={_details => resetState()} onError={logReactError}>
              <Routes>
                {views[0] !== undefined && <Route path='/' element={<Navigate to={views[0].path} />} />}
                {views.map((view, index) => <Route key={index} path={view.path} element={<view.component />} />)}
              </Routes>
            </ErrorBoundary>
          </AppContext.Provider>
          <ScrollToTop scroller={scroller} bottom={20} />
        </SimpleBar>
      </AppShellMain>
    </AppShell>
  </>;
}
