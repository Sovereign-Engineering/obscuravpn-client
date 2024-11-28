import { ActionIcon, AppShell, AppShellAside, AppShellHeader, AppShellMain, AppShellNavbar, AppShellSection, Burger, Button, Divider, Group, Image, Modal, Space, Text, Title, useComputedColorScheme, useMantineColorScheme } from '@mantine/core';
import { useDisclosure, useHotkeys, useInterval } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import React, { useEffect, useRef, useState } from 'react';
import { ErrorBoundary } from 'react-error-boundary';
import { Trans, useTranslation } from 'react-i18next';
import { BsMoonStarsFill } from 'react-icons/bs';
import { IoLogOutOutline, IoSunnySharp } from 'react-icons/io5';
import { NavLink, Navigate, Route, Routes, useNavigate } from 'react-router-dom';
import SimpleBar from 'simplebar-react';
import 'simplebar-react/dist/simplebar.min.css';
// src imports
import AppIcon from '../../apple/client/Assets.xcassets/AppIcon.appiconset/icon_128x128.png';
import classes from './App.module.css';
import { AppContext, ConnectingStrings, ExitsContext } from './common/appContext';
import { useLoadable } from './common/useLoadable';
import { NOTIF_VPN_DISCONNECT_CONNECT } from './common/notifIds';
import { HEADER_TITLE, IS_DEVELOPMENT, IS_WK_WEB_VIEW, VERSION, getLatestState, trueTypeOf, useCookie } from './common/utils';
import LanguageHeaders from './components/LanguageHeaders';
import { ScrollToTop } from './components/ScrollToTop';
import { UserPrefs } from './components/UserPrefs';
import * as commands from './tauri/commands';
import { tauriLogError, useSystemContext } from './tauri/SystemProvider';
// imported views need to be added to the `views` list variable
import { Account, Connection, DeveloperView, FallbackAppRender, Help, Location, LogIn, Settings, SplashScreen } from './views';

// defined in Rust side
const DAEMON_UNRESPONSIVE = 'DaemonUnresponsive';
const NOTIF_UPDATE = 'updateNotif';

export default function () {
  const { t, i18n } = useTranslation();
  // check if using custom titlebar to adjust other components
  const { usingCustomTitleBar, osPlatform, loading: systemProviderLoading } = useSystemContext();

  // Boilerplate State
  const navigate = useNavigate();
  const { toggleColorScheme } = useMantineColorScheme();
  useHotkeys([[osPlatform === 'darwin' ? 'mod+J' : 'ctrl+J', toggleColorScheme]]);
  const colorScheme = useComputedColorScheme();
  const [mobileNavOpened, { toggle: toggleMobileNav }] = useDisclosure();
  const [desktopNavOpened, setDesktopNavOpened] = useCookie('desktop-nav-opened', true);
  const toggleDesktopNav = () => setDesktopNavOpened(o => !o);

  // MISCELLANEOUS
  const updateInProgress = useRef(false);
  const scrollbarRef = useRef(null);

  // App State
  const [vpnConnected, setVpnConnected] = useState(false);
  const [connectionInProgress, setConnectionInProgress] = useState();
  const [warningNotices, setWarningNotices] = useState([]);
  const [importantNotices, setImportantNotices] = useState([]);
  const [appStatus, setStatus] = useState(null);
  const [osStatus, setOsStatus] = useState(null);
  const ignoreConnectingErrors = useRef(false);

  const views = [
    { component: Connection, path: '/connection', name: t('Connection') },
    { component: DeveloperView, path: '/developer', name: 'Developer' },
    { component: Location, path: '/location', name: t('Location') },
    { component: Account, path: '/account', name: t('Account') },
    { component: Help, path: '/help', name: t('Help') },
    { component: Settings, path: '/settings', name: t('Settings') },
  ];

  const isLoggedIn = !!appStatus?.accountId;
  const showAccountCreation = appStatus?.inNewAccountFlow;
  const loading = appStatus === null || osStatus === null || systemProviderLoading;

  useEffect(() => {
    // reminder: errors are auto logged
    commands.notices().then(notices => {
      const warnNotices = [];
      const importantNotices = [];
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

  async function tryConnect(exit = null, changingLocation = false) {
    if (!changingLocation) {
      setConnectionInProgress(ConnectingStrings.connecting);
    }
    ignoreConnectingErrors.current = false;
    try {
      await commands.connect(exit);
    } catch (e) {
      if (!ignoreConnectingErrors.current && e.message !== 'tunnelNotDisconnected') {
        notifications.hide('vpnError');
        notifications.show({ title: t('Error Connecting'), message: t('vpnError-' + e.message), color: 'red', id: 'vpnError', autoClose: false });
        // see https://linear.app/soveng/issue/OBS-775/not-starting-tunnel-because-it-isnt-disconnected-connecting#comment-e98a7150
        setConnectionInProgress(ConnectingStrings.UNSET);
      }
    }
  }

  async function disconnectFromVpn() {
    ignoreConnectingErrors.current = true;
    setConnectionInProgress(ConnectingStrings.disconnecting);
    setVpnConnected(false);
    await commands.disconnect();
  }

  async function toggleVpnConnection() {
    // this function no longer set the connection state
    // due to the backend command being async and not synchronous with status
    const tryDisconnect = vpnConnected || connectionInProgress === ConnectingStrings.connecting || connectionInProgress === ConnectingStrings.reconnecting;
    if (tryDisconnect) {
      await disconnectFromVpn();
    } else {
      await tryConnect()
    }
  }

  async function disconnectThenConnect(exitId) {
    if (vpnConnected) {
      setConnectionInProgress(ConnectingStrings.changingLocations);
      await commands.disconnectBlocking();
      notifications.update({
        id: NOTIF_VPN_DISCONNECT_CONNECT,
        color: 'white',
        autoClose: 1000
      });
      await tryConnect(exitId, true);
    }
  }

  const [platform, setPlatform] = useState(IS_WK_WEB_VIEW ? 'macos' : undefined);

  function notifyVpnError(errorEnum) {
    // see enum JsVpnError in commands.swift
    if (errorEnum !== null) {
      notifications.hide('vpnError');
      notifications.show({
        id: 'vpnError',
        withCloseButton: false,
        color: 'red',
        title: t('Error'),
        message: t(`vpnError-${errorEnum}`),
        autoClose: 15_000
      });
    }
  }

  function handleNewStatus(newStatus) {
    const vpnStatus = newStatus.vpnStatus;
    if (vpnStatus === undefined) return;

    if (vpnStatus.connected !== undefined) {
      setVpnConnected(true);
      setConnectionInProgress();
      notifications.hide('vpnError');
      notifications.update({
        id: NOTIF_VPN_DISCONNECT_CONNECT,
        color: 'green',
        autoClose: 1000
      });
    } else if (vpnStatus.connecting !== undefined) {
      setVpnConnected(false);
      setConnectionInProgress(value => {
        if (value === ConnectingStrings.changingLocations) return value;
        return ConnectingStrings.connecting;
      });
    } else if (vpnStatus.reconnecting !== undefined) {
      setConnectionInProgress(ConnectingStrings.reconnecting);
      if (vpnStatus.reconnecting.err !== undefined) {
        console.error(`got error while reconnecting: ${vpnStatus.reconnecting.err}`);
        notifyVpnError(vpnStatus.reconnecting.err);
      }
    } else if (vpnStatus.disconnected !== undefined) {
      setConnectionInProgress(value => {
        if (value === ConnectingStrings.changingLocations) return value;
        return ConnectingStrings.UNSET;
      });
      setVpnConnected(false);
    }

    if (platform === undefined) {
      setPlatform(newStatus?.platform);
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
        } catch (e) {
          console.error('command status failed', e.message || e.type);
          notifications.show({ title: t('Error') + ' ' + t(e.type || t('Fetching Status')), message: e.message, color: 'red' });
          // if (e.type === 'Unauthorized') {
          //   try { await commands.logout(); } catch { }
          // }
        }
      }
    })();
    return () => keepAlive = false;
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
        } catch (e) {
          console.error('command osStatus failed', e.message || e.type);
          notifications.show({ title: t('Error') + ' ' + t(e.type || t('Fetching OsStatus')), message: e.message, color: 'red' });
        }
      }
    })();
    return () => keepAlive = false;
  }, []);

  useEffect(() => {
    if (appStatus !== null) handleNewStatus(appStatus);
  }, [appStatus]);

  function resetState(details) {
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
      const onNavUpdate = e => navigate(`/${e.detail}`);
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
      <NavLink align='left' to={view.path} key={index} end={view.exact} onClick={() => toggleMobileNav(false)}
        className={({ isActive }) => classes.navLink + ' ' + (isActive ? classes.navLinkActive : classes.navLinkInactive)}>
        {/* TODO: Icons */}
        <Group><Text>{view.name ? view.name : view.name}</Text></Group>
      </NavLink>
    );
  }

  // hack for global styling the vertical simplebar based on state
  useEffect(() => {
    const el = document.getElementsByClassName('simplebar-vertical')[0];
    if (el !== undefined) {
      el.style.marginTop = usingCustomTitleBar ? '100px' : '70px';
      el.style.marginBottom = 0;
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
  } = useLoadable({
    skip: !osStatus?.internetAvailable || !isLoggedIn,
    load: commands.getAccount,
    periodMs: showAccountCreation ? 3600 * 1000 : 12 * 3600 * 1000,
    returnError: true,
  });

  useEffect(() => {
    if (accountInfoError) {
      console.error("Failed to fetch account info", accountInfoError);
      // We just ignore errors, they will be shown if the user goes to the account page.
    }
  }, [accountInfoError]);

  const appContext = {
    accountInfo: accountInfo ?? null,
    appStatus,
    connectionInProgress,
    osStatus,
    pollAccount,
    toggleVpnConnection,
    vpnConnect: tryConnect,
    vpnConnected,
    vpnDisconnect: disconnectFromVpn,
    vpnDisconnectConnect: disconnectThenConnect,
  }

  const exitsContext = {
    exitList: exitList ?? null,
    fetchExitList,
  }

  if (loading) return <SplashScreen text={systemProviderLoading ? 'Tauri loading' : t('appStatusLoading')} />;

  if (!isLoggedIn || showAccountCreation) return <LogIn accountNumber={appStatus?.accountId} accountActive={accountInfo?.active} />;

  // <> is an alias for <React.Fragment>
  return <>
    {/* non closable notice */}
    <Modal size='100%' overlayProps={{ backgroundOpacity: '70%' }} title={<Title order={5} style={{ color: 'orangered', letterSpacing: 1.5, textDecoration: 'bold' }}>{t('IMPORTANT NOTICE', { count: importantNotices.length })}</Title>} opened={importantNotices.length > 0} withCloseButton={false}>
      {importantNotices.map(notice => <Text style={{ marginBottom: 10 }}><Trans i18nKey='importantNotice' values={{ notice, count: importantNotices.length }} /></Text>)}
    </Modal>
    <AppShell padding='md'
      header={{ height: IS_WK_WEB_VIEW ? 0 : 60 }}
      navbar={IS_WK_WEB_VIEW ? {} : { width: 200, breakpoint: 'sm', collapsed: { mobile: !mobileNavOpened, desktop: !desktopNavOpened } }}
      aside={{ width: 200, breakpoint: 'sm', collapsed: { desktop: true, mobile: true } }}
      className={classes.appShell}>
      <AppShellMain p={0}>
        {usingCustomTitleBar && <Space h='xl' />}
        <SimpleBar scrollableNodeProps={{ ref: scrollbarRef }} autoHide={false} className={classes.simpleBar}>
          <UserPrefs>
            <AppContext.Provider value={appContext}>
              <ExitsContext.Provider value={exitsContext}>
                <ErrorBoundary FallbackComponent={FallbackAppRender} onReset={(details) => resetState(details)} onError={tauriLogError}>
                  <Routes>
                    <Route exact path='/' element={<Navigate to={views[0].path} />} />
                    {views.map((view, index) => <Route key={index} exact={view.exact} path={view.path} element={<view.component />} />)}
                  </Routes>
                </ErrorBoundary>
              </ExitsContext.Provider>
            </AppContext.Provider>
          </UserPrefs>
          <ScrollToTop scroller={scrollbarRef.current} bottom={20} />
        </SimpleBar>
      </AppShellMain>

      {!IS_WK_WEB_VIEW && <AppShellHeader data-tauri-drag-region p='md' className={classes.header}>
        <Group h='100%'>
          <Burger hiddenFrom='sm' opened={mobileNavOpened} onClick={toggleMobileNav} size='sm' />
          <Burger visibleFrom='sm' opened={desktopNavOpened} onClick={toggleDesktopNav} size='sm' />
          <Image src={AppIcon} w={28} />
          <Text>{HEADER_TITLE}</Text>
        </Group>

        <Group className={classes.headerRightItems} h='110%'>
          <LanguageHeaders i18n={i18n} />

          {IS_DEVELOPMENT &&
            <ActionIcon title='logout' variant='default' onClick={() => commands.logout().catch(e => notifications.show({ title: 'logoutFailed', message: e.type === 'logoutFailed' ? t('pleaseReportError') : '' }))
            } size={30}>
              <IoLogOutOutline />
            </ActionIcon>}

          <ActionIcon id='toggle-theme' title={osPlatform === 'darwin' ? '⌘ + J' : 'ctrl + J'} variant='default' onClick={() => toggleColorScheme()} size={30}>
            {colorScheme === 'dark' ? <IoSunnySharp size='1.5em' /> : <BsMoonStarsFill />}
          </ActionIcon>
        </Group>
      </AppShellHeader>}

      {!IS_WK_WEB_VIEW && <AppShellNavbar className={classes.titleBarAdjustedHeight} height='100%' width={{ sm: 200 }} p='xs' hidden={!mobileNavOpened}>
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

      <AppShellAside className={classes.titleBarAdjustedHeight} p='md' width={{ sm: 200, lg: 300 }}>
        <Text>Right Side. Use for help, support, quick action menu? For example, if we were building a trading app, we could use the aside for the trade parameters while leaving the main UI with the data</Text>
      </AppShellAside>
    </AppShell>
  </>;
}

function StartDaemonButton({ t }) {
  return <Button onClick={() => commands.start_daemon().then(() =>
    notifications.update({
      id: DAEMON_UNRESPONSIVE,
      title: t('daemonStarted'),
      message: t('postDaemonStartMsg'),
      color: 'green'
    })
  ).catch(e => {
    notifications.update({ id: DAEMON_UNRESPONSIVE, message: t(e.type) })
  })}>{t('startDaemon')}</Button>;
}

function getTimeStamp() {
  const currentDate = new Date();
  let hours = currentDate.getHours();
  var ampm = hours >= 12 ? 'pm' : 'am';
  let minutes = currentDate.getMinutes();
  minutes = minutes < 10 ? '0' + minutes : minutes;
  const timeStamp = `${hours}:${minutes} ${ampm}`;
  return timeStamp;
}
