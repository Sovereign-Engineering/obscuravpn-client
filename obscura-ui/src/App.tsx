import { AppShell, AppShellHeader, AppShellMain, AppShellNavbar, AppShellSection, Burger, Divider, Group, Image, Modal, Space, Text, Title, useComputedColorScheme, useMantineColorScheme } from '@mantine/core';
import { useDisclosure, useHotkeys, useThrottledValue } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { ReactNode, useEffect, useRef, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { Trans, useTranslation } from 'react-i18next';
import { Navigate, NavLink, Route, Routes, useNavigate } from 'react-router-dom';
import SimpleBar from 'simplebar-react';
import 'simplebar-react/dist/simplebar.min.css';
import AppIcon from '../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import classes from './App.module.css';
import * as commands from './bridge/commands';
import { logReactError, useSystemContext } from './bridge/SystemProvider';
import { Exit } from './common/api';
import { AppContext, AppStatus, ConnectionInProgress, ExitsContext, OsStatus } from './common/appContext';
import { fmtErrorI18n, fmtVpnError } from './common/danger';
import { NotificationId } from './common/notifIds';
import { useAsync } from './common/useAsync';
import { useLoadable } from './common/useLoadable';
import { HEADER_TITLE, IS_WK_WEB_VIEW, MIN_LOAD_MS, normalizeError, useCookie } from './common/utils';
import { ScrollToTop } from './components/ScrollToTop';
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
  // check if using custom titlebar to adjust other components
  const { usingCustomTitleBar, osPlatform, loading: systemProviderLoading } = useSystemContext();

  // Boilerplate State
  const navigate = useNavigate();
  const { toggleColorScheme } = useMantineColorScheme();
  useHotkeys([[osPlatform === 'darwin' ? 'mod+J' : 'ctrl+J', toggleColorScheme]]);
  const colorScheme = useComputedColorScheme();
  const [mobileNavOpened, { toggle: toggleMobileNav }] = useDisclosure();
  const [desktopNavOpenedCookie, setDesktopNavOpenedCookie] = useCookie('desktop-nav-opened', 'true');
  const desktopNavOpened = desktopNavOpenedCookie === 'true';
  const toggleDesktopNav = () => setDesktopNavOpenedCookie(o => o === 'true' ? 'false' : 'true');
  const [scroller, setScroller] = useState<HTMLElement | null>(null);

  // App State
  const [vpnConnected, setVpnConnected] = useState(false);
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
  const loading = appStatus === null || osStatus === null || systemProviderLoading;

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

  async function tryConnect(exit: string | null = null, changingLocation = false) {
    if (!changingLocation) {
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

  async function toggleVpnConnection() {
    // this function no longer set the connection state
    // due to the backend command being async and not synchronous with status
    const tryDisconnect = vpnConnected || connectionInProgress === ConnectionInProgress.Connecting || connectionInProgress === ConnectionInProgress.Reconnecting;
    if (tryDisconnect) {
      await disconnectFromVpn();
    } else {
      await tryConnect()
    }
  }

  async function disconnectThenConnect(exitId: string) {
    if (vpnConnected) {
      setConnectionInProgress(ConnectionInProgress.ChangingLocations);
      await commands.disconnectBlocking();
      notifications.update({
        id: NotificationId.VPN_DISCONNECT_CONNECT,
        color: 'white',
        autoClose: 10_000,
        // keep same message
        message: undefined
      });
      await tryConnect(exitId, true);
    }
  }

  function notifyVpnError(errorEnum: string) {
    // see enum JsVpnError in commands.swift
    if (errorEnum !== null) {
      notifications.hide(NotificationId.VPN_ERROR);
      notifications.show({
        id: NotificationId.VPN_ERROR,
        withCloseButton: false,
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
      setConnectionInProgress(value => {
        if (value === ConnectionInProgress.ChangingLocations) return value;
        return ConnectionInProgress.Connecting;
      });
    } else if (vpnStatus.reconnecting !== undefined) {
      setVpnConnected(false);
      setConnectionInProgress(ConnectionInProgress.Reconnecting);
      if (vpnStatus.reconnecting.err !== undefined) {
        console.error(`got error while reconnecting: ${vpnStatus.reconnecting.err}`);
        notifyVpnError(vpnStatus.reconnecting.err);
      }
    } else if (vpnStatus.disconnected !== undefined) {
      setConnectionInProgress(value => {
        if (value === ConnectionInProgress.ChangingLocations) return value;
        return ConnectionInProgress.UNSET;
      });
      setVpnConnected(false);
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

  function resetState() {
    // Useful for runtime rendering exceptions
    if (vpnConnected) toggleVpnConnection();
    if (window.location.pathname === '/connection') {
      window.location.pathname = '/logs';
    } else {
      window.location.pathname = '/';
    }
  }

  // native driven navigation
  useEffect(() => {
    if (IS_WK_WEB_VIEW) {
      const onNavUpdate = (e: Event) => {
        if (e instanceof CustomEvent) {
          navigate(`/${e.detail}`);
        } else {
          console.error('expected custom event for navigation purposes, got generic Event');
        }
      };
      window.addEventListener('navUpdate', onNavUpdate);
      return () => window.removeEventListener('navUpdate', onNavUpdate);
    }
  }, []);

  const onPaymentSucceeded = () => {
    console.log("handling paymentSucceeded event");
    void pollAccount();
    commands.setInNewAccountFlow(false);
  }

  // deep link payment succeeded
  useEffect(() => {
    if (IS_WK_WEB_VIEW) {
      window.addEventListener('paymentSucceeded', onPaymentSucceeded);
      return () => window.removeEventListener('paymentSucceeded', onPaymentSucceeded);
    }
  }, []);

  function NavLinks() {
    // TODO: useHotkeys and abstract this
    return views.map((view, index) =>
      <NavLink to={view.path} key={index} end={view.exact} onClick={() => toggleMobileNav()}
        className={({ isActive }) => classes.navLink + ' ' + (isActive ? classes.navLinkActive : classes.navLinkInactive)}>
        {/* TODO: Icons */}
        <Group><Text>{view.name ? view.name : view.name}</Text></Group>
      </NavLink>
    );
  }

  // hack for global styling the vertical simplebar based on state
  useEffect(() => {
    const el = document.getElementsByClassName('simplebar-vertical')[0];
    if (el instanceof HTMLElement) {
      el.style.marginTop = usingCustomTitleBar ? '100px' : '70px';
      el.style.marginBottom = '0px';
    }
  }, [usingCustomTitleBar]);

  const {
    lastSuccessfulValue: exitList,
    error: exitListError,
    refresh: fetchExitList,
  } = useLoadable({
    skip:
      !osStatus?.internetAvailable
      || !isLoggedIn, // The API client currently fails all requests until logged in even if they don't require auth.
    load: commands.getExitServers,
    periodMs: 12 * 3600 * 1000,
    returnError: true,
  });

  useEffect(() => {
    if (exitListError) {
      console.error("Failed to fetch exits", exitListError);

      // We just ignore errors, they will be shown if the user goes to a page that displays exits.
    }
  }, [exitListError]);

  const {
    lastSuccessfulValue: accountInfo,
    error: accountInfoError,
    refresh: pollAccount,
    loading: accountLoading
  } = useLoadable({
    skip: !osStatus?.internetAvailable || !isLoggedIn,
    load: commands.getAccount,
    periodMs: showAccountCreation ? 3600 * 1000 : 12 * 3600 * 1000,
    returnError: true,
  });
  const accountLoadingDelayed = useThrottledValue(accountLoading, accountLoading ? MIN_LOAD_MS : 0);

  useEffect(() => {
    if (accountInfoError) {
      console.error("Failed to fetch account info", accountInfoError);
      // We just ignore errors, they will be shown if the user goes to the account page.
    }
  }, [accountInfoError]);

  const {
    error: checkUpdateError,
  } = useAsync({
    skip: !osStatus?.internetAvailable,
    load: commands.checkForUpdates,
    returnError: true,
  });

  useEffect(() => {
    const error = normalizeError(checkUpdateError);
    const message = error instanceof commands.CommandError
      ? fmtErrorI18n(t, error) : error.message;
    notifications.show({
      color: 'red',
      title: t('Error'),
      message
    });
  }, [checkUpdateError]);

  if (loading) return <SplashScreen text={systemProviderLoading ? t('synchronizing') : t('appStatusLoading')} />;

  if (!isLoggedIn || showAccountCreation) return <LogIn accountNumber={appStatus.accountId} accountActive={accountInfo?.active} />;

  const appContext = {
    accountInfo: accountInfo ?? null,
    appStatus,
    connectionInProgress,
    osStatus,
    pollAccount,
    accountLoading: accountLoadingDelayed,
    toggleVpnConnection,
    vpnConnect: tryConnect,
    vpnConnected,
    vpnDisconnect: disconnectFromVpn,
    vpnDisconnectConnect: disconnectThenConnect,
  }

  const exitsContext = {
    exitList: exitList as Exit[] ?? null,
    fetchExitList,
  }

  // <> is an alias for <React.Fragment>
  return <>
    {/* non-closable notice */}
    <Modal size='100%' overlayProps={{ backgroundOpacity: 0.7 }} opened={importantNotices.length > 0} withCloseButton={false} onClose={() => { }}
      title={<Title order={5} style={{ color: 'orangered', letterSpacing: 1.5, textDecoration: 'bold' }}>{t('IMPORTANT NOTICE', { count: importantNotices.length })}</Title>}>
      {importantNotices.map(notice => <Text style={{ marginBottom: 10 }}><Trans i18nKey='importantNotice' values={{ notice, count: importantNotices.length }} /></Text>)}
    </Modal>
    <AppShell padding='md'
      header={{ height: IS_WK_WEB_VIEW ? 0 : 60 }}
      navbar={IS_WK_WEB_VIEW ? undefined : { width: 200, breakpoint: 'sm', collapsed: { mobile: !mobileNavOpened, desktop: !desktopNavOpened } }}
      className={classes.appShell}>
      <AppShellMain p={0}>
        {usingCustomTitleBar && <Space h='xl' />}
        <SimpleBar scrollableNodeProps={{ ref: setScroller }} autoHide={false} className={classes.simpleBar}>
          <AppContext.Provider value={appContext}>
            <ExitsContext.Provider value={exitsContext}>
              <ErrorBoundary FallbackComponent={FallbackAppRender} onReset={_details => resetState()} onError={logReactError}>
                <Routes>
                  {views[0] !== undefined && <Route path='/' element={<Navigate to={views[0].path} />} />}
                  {views.map((view, index) => <Route key={index} path={view.path} element={<view.component />} />)}
                </Routes>
              </ErrorBoundary>
            </ExitsContext.Provider>
          </AppContext.Provider>
          <ScrollToTop scroller={scroller} bottom={20} />
        </SimpleBar>
      </AppShellMain>

      {!IS_WK_WEB_VIEW && <AppShellHeader data-tauri-drag-region p='md' className={classes.header}>
        <Group h='100%'>
          <Burger hiddenFrom='sm' opened={mobileNavOpened} onClick={toggleMobileNav} size='sm' />
          <Burger visibleFrom='sm' opened={desktopNavOpened} onClick={toggleDesktopNav} size='sm' />
          <Image src={AppIcon} w={28} />
          <Text>{HEADER_TITLE}</Text>
        </Group>
      </AppShellHeader>}

      {!IS_WK_WEB_VIEW && <AppShellNavbar className={classes.titleBarAdjustedHeight} h='100%' w={{ sm: 200 }} p='xs' hidden={!mobileNavOpened}>
        <AppShellSection grow><NavLinks /></AppShellSection>
        {/* Bottom of Navbar Example: https://mantine.dev/app-shell/?e=NavbarSection */}
        <AppShellSection>
          {warningNotices.length > 0 && <>
            <Divider m={10} label={<Title style={{ color: 'orange', letterSpacing: 2 }} order={5}>{t('WARNING', { count: warningNotices.length })}</Title>} />
            {
              warningNotices.map((notice, i) => <Text key={i} style={{ color: 'orange' }}>
                <Trans i18nKey='warningNotice' values={{ notice, count: warningNotices.length }} />
              </Text>)
            }
          </>}
        </AppShellSection>
      </AppShellNavbar>}
    </AppShell>
  </>;
}
