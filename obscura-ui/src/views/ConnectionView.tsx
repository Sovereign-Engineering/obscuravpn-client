import { Anchor, Button, Combobox, Divider, Flex, Group, Image, Paper, Progress, ProgressRootProps, ScrollArea, Space, Stack, Text, ThemeIcon, Title, useCombobox, useMantineTheme } from '@mantine/core';
import { useFocusTrap, useInterval, useToggle } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { ReactNode, useContext, useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { BsChevronDown, BsPinFill } from 'react-icons/bs';
import { IoIosEyeOff } from 'react-icons/io';
import { MdLanguage, MdLaptopMac, MdOutlineWifiOff } from 'react-icons/md';
import { ExitSelector, ExitSelectorCity } from 'src/bridge/commands';
import * as ObscuraAccount from '../common/accountUtils';
import { accountIsExpired, Exit, getContinent, getExitCountry, useReRenderWhenExpired } from '../common/api';
import { AppContext, ConnectionInProgress, getCityFromStatus, isConnecting, NEVPNStatus, PinnedLocation, useIsConnecting, useIsTransitioning } from '../common/appContext';
import commonClasses from '../common/common.module.css';
import { exitCityEquals, exitLocation, exitsSortComparator, getCountryFlag, getExitCountryFlag } from '../common/exitUtils';
import { KeyedSet } from '../common/KeyedSet';
import { useExitList } from '../common/useExitList';
import { useCookie } from '../common/utils';
import BoltBadgeAuto from '../components/BoltBadgeAuto';
import { CColorSchemeContext } from '../components/CachedColorScheme';
import ExternalLinkIcon from '../components/ExternalLinkIcon';
import ObscuraChip from '../components/ObscuraChip';
import DecoConnected from '../res/deco/deco-connected.svg';
import DecoConnectingDark1 from '../res/deco/deco-connecting-dark-1.svg';
import DecoConnectingDark2 from '../res/deco/deco-connecting-dark-2.svg';
import DecoConnectingDark3 from '../res/deco/deco-connecting-dark-3.svg';
import DecoConnectingLight1 from '../res/deco/deco-connecting-light-1.svg';
import DecoConnectingLight2 from '../res/deco/deco-connecting-light-2.svg';
import DecoConnectingLight3 from '../res/deco/deco-connecting-light-3.svg';
import DecoDisconnectedDark from '../res/deco/deco-disconnected-dark.svg';
import DecoDisconnectedLight from '../res/deco/deco-disconnected-light.svg';
import DecoOfflineDark from '../res/deco/deco-offline-dark.svg';
import DecoOfflineLight from '../res/deco/deco-offline-light.svg';
import MascotConnectedFirstTime from '../res/mascots/connected-first-time-mascot.svg';
import MascotConnected from '../res/mascots/connected-mascot.svg';
import MascotConnecting1 from '../res/mascots/connecting-1-mascot.svg';
import MascotConnecting2 from '../res/mascots/connecting-2-mascot.svg';
import MascotConnecting3 from '../res/mascots/connecting-3-mascot.svg';
import MascotConnecting4 from '../res/mascots/connecting-4-mascot.svg';
import MascotDead from '../res/mascots/dead-mascot.svg';
import MascotNotConnected from '../res/mascots/not-connected-mascot.svg';
import MascotValidating from '../res/mascots/validating-mascot.svg';
import ObscuraIconHappy from '../res/obscura-icon-happy.svg';
import classes from './ConnectionView.module.css';

// Los Angeles, CA
const BUTTON_WIDTH = 320;

export default function Connection() {
    const { t } = useTranslation();
    const { vpnConnected, initiatingExitSelector, connectionInProgress, osStatus, appStatus, showOfflineUI } = useContext(AppContext);
    const connectionTransition = useIsTransitioning();

    const { account } = appStatus;
    const { exitList } = useExitList({
        periodS: 3600,
    });
    const accountInfo = account?.account_info ?? null;

    useReRenderWhenExpired(account);

    const targetCity = getCityFromStatus(appStatus.vpnStatus);
    const specificInitiation = !(initiatingExitSelector === undefined || 'any' in initiatingExitSelector);
    const isCityConnect = (targetCity !== undefined || specificInitiation);
    const accountHasExpired = accountInfo !== null && accountIsExpired(accountInfo);

    /* If quick connect is used, don't show the combobox while connecting to avoid confusion.
      This is because we don't want the user to think they are connecting to the last chosen location.
      It's possible in the future that we can propagate which location is being connected to while connecting.
      Additionally, we don't want to show the dropdown when not connected AND a connection pre-requisite is missing.
      If we're already connected and the account expires, we still want to show the disconnect.
      Only when we're no longer connected should the pre-requisite take precedence.
    */
    const showLocationSelect = (isCityConnect || osStatus.osVpnStatus !== NEVPNStatus.Connecting) && (vpnConnected || accountInfo !== null && !accountHasExpired);
    /* The primary button is hidden in the following scenarios:
      1) connected
      2) not connecting via quick connect (i.e. connecting to a user-selected location)
      3) disconnecting
    */
    const primaryButtonShown = !(vpnConnected || (isCityConnect && osStatus.osVpnStatus !== NEVPNStatus.Disconnected) || osStatus.osVpnStatus === NEVPNStatus.Disconnecting);

    const getTitle = () => {
        if (accountHasExpired) return t('account-Expired');
        if (connectionTransition) {
            const sampleExit = targetCity && exitList?.find(e => e.city_code == targetCity.city_code && e.country_code == targetCity.country_code);

            switch (connectionInProgress) {
                case ConnectionInProgress.Connecting:
                case ConnectionInProgress.ChangingLocations:
                    return t('connectingTo', { location: sampleExit?.city_name ?? 'Obscura' });
                case ConnectionInProgress.Reconnecting:
                    return t('reconnectingObscura');
                case ConnectionInProgress.Disconnecting:
                    return t('Disconnecting');
            }
        }
        if (vpnConnected) return t('connectedToObscura');
        if (showOfflineUI) return t('disconnected');
        if (accountInfo === null) return t('validatingAccount')
        return t('notConnected');
    }

    const Subtitle = () => {
        if (vpnConnected && !connectionTransition) return t('enjoyObscura');
        if (accountHasExpired) return t('continueUsingObscura');
        if (connectionTransition) return t('pleaseWaitAMoment');
        if (showOfflineUI) return t('connectToInternet');
        if (accountInfo === null) return '';
        return t('connectToEnjoy');
    }

    return (
        <Stack align='center' h='100vh' gap={0} className={classes.container} style={{ backgroundImage: `url("${Deco()}")`}}>
            <Space h={40} />
            <Mascot />
            <Stack align='center' gap={primaryButtonShown ? 0 : 20} mt={primaryButtonShown ? 0 : 20} justify='space-around'>
                <Title order={2} fw={600}>{getTitle()}</Title>
                <Title order={4} mt={5} h='xl' fw={350} className={classes.subtitle}>{Subtitle()}</Title>
            </Stack>
            <Space h='xs' />
            <PrimaryConnectButton />
            <Space h='xs' />
            {showLocationSelect && <LocationSelect />}
            {
                vpnConnected && connectionInProgress === ConnectionInProgress.UNSET && <div className={classes.checkMyConnection}>
                    <Space />
                    <Text className={classes.checkMyConnection}><Anchor c='inherit' href={ObscuraAccount.CHECK_STATUS_WEBPAGE} underline='always'>{t('checkMyConnection')}</Anchor> <ExternalLinkIcon size={12} style={{ marginBottom: -1 }} /></Text>
                </div>
            }
            <div style={{ flexGrow: 1 }} />
            <ConnectionProgressBar />
            <Space />
        </Stack >
    );
}

/**
 * One of: Quick Connect, Manage Account, Cancel Connecting, Disconnecting (disabled)
 */
function PrimaryConnectButton() {
  const { t } = useTranslation();
  const theme = useMantineTheme();
  const { vpnConnected, connectionInProgress, initiatingExitSelector, osStatus, vpnConnect, vpnDisconnect, appStatus } = useContext(AppContext);
  const { internetAvailable } = osStatus;
  const { account } = appStatus;
  const connectionTransition = connectionInProgress !== ConnectionInProgress.UNSET;
  const inConnectingState = useIsConnecting();

  const accountInfo = account?.account_info ?? null;
  const accountHasExpired = accountInfo !== null && accountIsExpired(accountInfo);

  const targetCity = getCityFromStatus(appStatus.vpnStatus);
  const specificInitiation = !(initiatingExitSelector === undefined || 'any' in initiatingExitSelector);
  const connectingToCity = (targetCity !== undefined || specificInitiation) && (osStatus.osVpnStatus !== NEVPNStatus.Disconnected || appStatus.vpnStatus.connecting !== undefined);

  const isReconnecting = osStatus.osVpnStatus === NEVPNStatus.Reasserting || appStatus.vpnStatus.connecting?.reconnecting;

  if (inConnectingState && (!connectingToCity || isReconnecting) && osStatus.osVpnStatus !== NEVPNStatus.Disconnecting) {
    return <>
      <Space h='lg' />
      <Button w={BUTTON_WIDTH} {...theme.other.buttonDisconnectProps} mt={5} onClick={vpnDisconnect}>{t('Cancel Connecting')}</Button>
    </>
  }

  if (!vpnConnected && accountInfo !== null && accountHasExpired) {
    return <Button component='a' href={ObscuraAccount.APP_ACCOUNT_TAB}>{t('ManageAccount')}</Button>;
  }

  const showQuickConnect = !vpnConnected && !connectingToCity && accountInfo !== null && !appStatus.vpnStatus.connecting && osStatus.osVpnStatus !== NEVPNStatus.Disconnecting;
  const qcBtnAction = (_: MouseEvent) => vpnConnected ? vpnDisconnect() : vpnConnect({ any: {} });
  const qcBtnDisabled = !internetAvailable || connectionTransition;
  const primaryBtnDisconnectProps = (vpnConnected && connectionInProgress !== ConnectionInProgress.Reconnecting) ? theme.other.buttonDisconnectProps : {};

  if (showQuickConnect) {
    const buttonContent = connectionTransition ? t(connectionInProgress) + '...' : <Group gap={5}><BoltBadgeAuto />{t('QuickConnect')}</Group>;
    return <Button size='md' className={commonClasses.button} onClick={qcBtnAction} w={BUTTON_WIDTH} disabled={qcBtnDisabled} {...primaryBtnDisconnectProps}>{buttonContent}</Button>
  }
}

function ConnectionProgressBar() {
    const { t } = useTranslation();
    const {
        vpnConnected,
        connectionInProgress,
        showOfflineUI
    } = useContext(AppContext);

    const progressWidth = { base: 40, xs: 50 };

    const connectingProgressBars = usePulsingProgress({ activated: isConnecting(connectionInProgress), bars: 2, w: progressWidth });
    return (
        <Paper shadow='xl' withBorder className={classes.connectionProgressBarContainer} maw={600} p={{base: 'xs', xs: 'md'}} pt={5} pb='xs' radius='lg'>
            <Group mih={50} className={classes.connectionProgressBarGroup} align='center'>
                <Stack gap='0' align='center'>
                    <ThemeIcon variant='transparent' c='white'>
                        <MdLaptopMac size={20} />
                    </ThemeIcon>
                    <Text size='xs' c='white'>{t('yourDevice')}</Text>
                </Stack>
                {
                    showOfflineUI &&
                    <>
                        <Progress className={classes.progress} w={80} value={0} h={2} />
                        <Stack gap='0' align='center'>
                            <ThemeIcon variant='transparent' c='red.6'>
                                <MdOutlineWifiOff size={22} />
                            </ThemeIcon>
                            <Text size='xs' c='red'>{t('noInternet')}</Text>
                        </Stack>
                        <Progress className={classes.progress} w={80} value={0} h={2} />
                    </>
                }
                {
                    !showOfflineUI && (connectionInProgress === ConnectionInProgress.Disconnecting || (!vpnConnected && connectionInProgress === ConnectionInProgress.UNSET)) &&
                    <Stack gap='xs' align='center' justify='flex-end' h={50} className={classes.trafficVulnerable}>
                        <Progress className={classes.trafficVulnerableProgressBar} value={100} color='red.6' h={2} />
                        <Text size='xs' c='red.6'>
                            {t('trafficVulnerable')}
                        </Text>
                    </Stack>
                }
                {
                    ((vpnConnected && connectionInProgress === ConnectionInProgress.UNSET) || isConnecting(connectionInProgress)) && <>
                    {connectingProgressBars[0]}
                    <Stack gap='0' align='center'>
                        <ThemeIcon variant='transparent' c='white'>
                            <Image src={ObscuraIconHappy} w={20} />
                        </ThemeIcon>
                        <Text size='xs' c='white'>Obscura</Text>
                    </Stack>
                    {connectingProgressBars[1]}
                    <Stack gap='0' align='center'>
                        <ThemeIcon variant='transparent' c={(vpnConnected && connectionInProgress !== ConnectionInProgress.ChangingLocations) ? 'white' : 'dimmed'}>
                            <IoIosEyeOff size={20} />
                        </ThemeIcon>
                        <Text size='xs' c={(vpnConnected && connectionInProgress !== ConnectionInProgress.ChangingLocations) ? 'white' : 'dimmed'}>Blind Relay</Text>
                    </Stack>
                    <Progress className={classes.progress} w={progressWidth} value={(vpnConnected && connectionInProgress !== ConnectionInProgress.ChangingLocations) ? 100 : 0} h={2} />
                </>}
                <Stack gap='0' align='center'>
                    <ThemeIcon variant='transparent' c={showOfflineUI ? 'red.6' : (connectionInProgress === ConnectionInProgress.UNSET ? 'white' : 'dimmed')}>
                        <MdLanguage size={20} />
                    </ThemeIcon>
                    <Text size='xs' c={showOfflineUI ? 'red.6' : (connectionInProgress === ConnectionInProgress.UNSET ? 'white' : 'dimmed')}>{t('Internet')}</Text>
                </Stack>
            </Group>
        </Paper>
    );
}

interface PulsingProgressProps extends ProgressRootProps {
  activated: boolean,
  bars: number,
}

function usePulsingProgress({ activated, bars = 2, w }: PulsingProgressProps) {
    const activeLength = 50;
    const segmentSize = activeLength / 2;
    const values = Array.from({ length: (bars + 1) * (100 / activeLength * bars) }, (_, i) => i * segmentSize);
    // show a pause for more natural feeling
    values.push(...Array(4).fill(values.at(-1)));

    const [value, toggleValue] = useToggle(values);

    const { start, stop } = useInterval(() => {
        toggleValue();
    }, 40);

    useEffect(() => {
        if (activated) {
            start();
        } else {
            stop();
        }
        return () => stop();
    }, [activated, start, stop]);

    const progressComponents: ReactNode[] = [];

    const ProgressSection = ({ value, threshold }: { value: number, threshold: number }) => {
        return <Progress.Section className={!activated || (value >= threshold - activeLength && value <= threshold) ? undefined : classes.progress} value={25} />
    }

    for (let index = 0; index < bars; index++) {
        progressComponents.push(
            <Progress.Root h={2} w={w} className={classes.progress} transitionDuration={50}>
                <ProgressSection value={value} threshold={activeLength + 100 * index} />
                <ProgressSection value={value} threshold={(activeLength + segmentSize) + 100 * index} />
                <ProgressSection value={value} threshold={(activeLength * 2) + (100 * index)} />
                <ProgressSection value={value} threshold={(segmentSize * 5) + (100 * index)} />
            </Progress.Root>
        );
    }
    progressComponents.push(values);
    return progressComponents;
}

const DECO_CONNECTING_ARRAY = {
    light: [DecoConnectingLight1, DecoConnectingLight2, DecoConnectingLight3],
    dark: [DecoConnectingDark1, DecoConnectingDark2, DecoConnectingDark3]
};
const DEC_LAST_IDX = DECO_CONNECTING_ARRAY.light.length - 1;

function Deco() {
    const {
        vpnConnected,
        connectionInProgress,
        showOfflineUI,
        osStatus
    } = useContext(AppContext);
    const colorScheme = useContext(CColorSchemeContext);
    const [connectingIndex, toggleConnectingDeco] = useToggle([0, 1, 2, 2]);

    const { start, stop } = useInterval(() => {
      toggleConnectingDeco();
    }, osStatus.osVpnStatus === NEVPNStatus.Disconnecting ? 750 : 500);

    useEffect(() => {
        if (connectionInProgress !== ConnectionInProgress.UNSET) {
            start();
        } else {
            if (connectingIndex !== 0) {
              toggleConnectingDeco(0);
            }
            stop();
        }
        return () => stop();
    }, [connectionInProgress, start, stop]);

    if (showOfflineUI) return colorScheme === 'light' ? DecoOfflineLight : DecoOfflineDark;

    if (connectionInProgress !== ConnectionInProgress.UNSET) {
        // reverse the animation when disconnecting
        const adjustedIdx = osStatus.osVpnStatus === NEVPNStatus.Disconnecting ? DEC_LAST_IDX - connectingIndex : connectingIndex;
        const connectionDeco = DECO_CONNECTING_ARRAY[colorScheme][adjustedIdx];
        if (connectionDeco === undefined) {
            console.error(`adjustedIdx/connectingIndex (${adjustedIdx} or ${connectingIndex}) longer than DECO_CONNECTING_ARRAY`);
            return DECO_CONNECTING_ARRAY[colorScheme][0];
        }
        return connectionDeco;
    };

    if (vpnConnected) return DecoConnected;
    return colorScheme === 'light' ? DecoDisconnectedLight : DecoDisconnectedDark;
}

const MASCOT_CONNECTING = [
    MascotConnecting1,
    MascotConnecting2,
    MascotConnecting3,
    MascotConnecting4
];

const ConnectedBefore = {
    NEVER: '0',
    FIRST_CONNECT: '1',
    YES: '2',
}

function Mascot() {
    const {
        vpnConnected,
        connectionInProgress,
        appStatus,
        showOfflineUI
    } = useContext(AppContext);
    const accountInfo = appStatus.account?.account_info ?? null;
    // tuned to show ... for 3 extra cycles
    const [connectingIndex, toggleConnectingDeco] = useToggle([0, 1, 2, 3, 3, 3, 3]);
    // want to show celebratory mascot the first time the user uses the app
    const isTransitioning = useIsTransitioning();
    const [connectedBefore, setConnectedBefore] = useCookie('connected-before', ConnectedBefore.NEVER);

    useEffect(() => {
        if (vpnConnected) {
            if (connectedBefore === ConnectedBefore.NEVER) {
                setConnectedBefore(ConnectedBefore.FIRST_CONNECT);
            }
        } else if (!vpnConnected && connectedBefore === ConnectedBefore.FIRST_CONNECT) {
            setConnectedBefore(ConnectedBefore.YES);
        }
    }, [vpnConnected]);

    const { start, stop } = useInterval(() => {
        toggleConnectingDeco();
    }, 120);

    useEffect(() => {
        if (connectionInProgress !== ConnectionInProgress.UNSET) {
            start();
        } else {
            if (connectingIndex !== 0) {
              toggleConnectingDeco(0);
            }
            console.log('stopped');
            stop();
        }
        return () => stop();
    }, [connectionInProgress]);

    const getMascot = () => {
        if (isTransitioning) {
            const mascotConnecting = MASCOT_CONNECTING[connectingIndex];
            if (mascotConnecting === undefined) {
                console.error(`unexpected mascot connectingIndex value ${connectingIndex}`);
                return MascotConnecting3;
            }
            return mascotConnecting;
        }
        if (showOfflineUI) return MascotDead;
        if (vpnConnected) return connectedBefore === ConnectedBefore.FIRST_CONNECT ? MascotConnectedFirstTime : MascotConnected;
        if (accountInfo === null) return MascotValidating;
        if (accountIsExpired(accountInfo)) return MascotDead;
        return MascotNotConnected;
    };
    return <Image src={getMascot()} maw={90} />;
}

function LocationSelect(): ReactNode {
    const { t } = useTranslation();

    const { exitList } = useExitList({
        periodS: 60,
    });
    const locations = useMemo(() => {
      return new KeyedSet(
        (exit: {country_code: string, city_code: string}) => JSON.stringify([exit.country_code, exit.city_code]),
        exitList,
      );
    }, [exitList]);

    const { appStatus, vpnConnect, vpnConnected, connectionInProgress, osStatus, showOfflineUI } = useContext(AppContext);
    const { internetAvailable } = osStatus;
    const { lastChosenExit, pinnedLocations } = appStatus;
    const connectedExit = appStatus.vpnStatus.connected?.exit;
    const pinnedLocationSet = new KeyedSet(
      (loc: {city_code: string, country_code: string}) => JSON.stringify([loc.country_code, loc.city_code]),
      pinnedLocations,
    );

    const [selectedCity, setSelectedCity] = useState<ExitSelectorCity | null>(null);
    const selectedExampleExit = selectedCity && locations.get(selectedCity);
    const isTransitioning = useIsTransitioning();

    useEffect(() => {
      if (connectedExit !== undefined) {
        setSelectedCity(connectedExit);
      } else if (isTransitioning) {
        const cityExit = getCityFromStatus(appStatus.vpnStatus);
        if (cityExit) {
          setSelectedCity(cityExit);
        }
        return;
      } else if (lastChosenExit && "city" in lastChosenExit) {
        setSelectedCity(lastChosenExit.city);
      } else if (pinnedLocations.length > 0) {
        let loc = pinnedLocations[0]!;
        setSelectedCity({
          city_code: loc.city_code,
          country_code: loc.country_code,
        });
      }
    }, [isTransitioning, appStatus, connectedExit, lastChosenExit, pinnedLocations]);

    // need to disable both combo (forces a collapsed dropdown) and button (non-clickable)
    const comboDisabled = !internetAvailable || connectionInProgress !== ConnectionInProgress.UNSET;
    const showLastChosenLabel = lastChosenExit
      && "city" in lastChosenExit
      && selectedCity?.city_code === lastChosenExit.city.city_code
      && selectedCity?.country_code === lastChosenExit.city.country_code
      && !!exitList
      && !vpnConnected
      && !isConnecting(connectionInProgress);
    const showPinned = selectedCity !== null
      && pinnedLocationSet.has(selectedCity)
      && (vpnConnected || isConnecting(connectionInProgress));

    const combobox = useCombobox({
        onDropdownClose: () => combobox.resetSelectedOption(),
    });
    const focusRef = useFocusTrap(combobox.dropdownOpened);

    return (
        <>
            <LocationConnectTopCaption />
            <Space />
            <Flex gap='xs' direction={{base: 'column', xs: 'row'}}>
                <Combobox
                    store={combobox}
                    position='bottom-start'
                    withArrow={false}
                    shadow='md'
                    disabled={comboDisabled}
                    size='lg'
                >
                    <Combobox.Target>
                        <Group ref={focusRef} gap={0} style={{ minWidth: BUTTON_WIDTH }}>
                            <Button
                                disabled={comboDisabled}
                                size='lg'
                                variant='default'
                                justify='space-between'
                                onClick={() => combobox.toggleDropdown()}
                                flex={1}
                                rightSection={<Group gap='xs'>
                                    {showLastChosenLabel && <ObscuraChip>{t('lastChosen')}</ObscuraChip>}
                                    {showPinned && <ThemeIcon variant='transparent' size={16} c='dimmed'><BsPinFill /></ThemeIcon>}
                                    <BsChevronDown
                                        size={16}
                                        style={{
                                            transform: combobox.dropdownOpened ? 'rotate(-180deg)' : undefined,
                                            transition: 'transform 200ms ease-in-out'
                                        }}
                                    />
                                </Group>}
                            >
                                {
                                  selectedCity === null
                                  ? <Text>{showOfflineUI ? t('noInternet') : t('selectLocation')}</Text>
                                  : <Group gap='xs'>
                                    <Text size='lg'>{getCountryFlag(selectedCity.country_code)} {selectedExampleExit?.city_name}</Text>
                                  </Group>
                                }
                            </Button>
                        </Group>
                    </Combobox.Target>

                    <Combobox.Dropdown>
                        <Combobox.Options pt={10}>
                            <ScrollArea.Autosize mah={250} type='always'>
                                <CityOptions locations={locations} onExitSelect={(country_code: string, city_code: string) => {
                                    combobox.closeDropdown();
                                    if (connectedExit?.country_code === country_code && connectedExit?.city_code === city_code) {
                                      return;
                                    }
                                    let location = locations.get({
                                        country_code,
                                        city_code,
                                    });
                                    if (!location) {
                                        console.error("No exit matching selected location", country_code, city_code);
                                        notifications.show({
                                            title: t('Error'),
                                            message: t('InternalError'),
                                            color: 'red',
                                        });
                                        return;
                                    }
                                    let city = {
                                        country_code,
                                        city_code,
                                    };
                                    setSelectedCity(city);
                                    vpnConnect({
                                      city,
                                    });
                                }} lastChosenExit={lastChosenExit} pinnedLocationSet={pinnedLocationSet} />
                            </ScrollArea.Autosize>
                        </Combobox.Options>
                    </Combobox.Dropdown>
                </Combobox>
                <LocationConnectRightButton dropdownOpened={combobox.dropdownOpened} selectedCity={selectedCity} />
            </Flex>
        </>
    );
}

function LocationConnectTopCaption() {
    const { t } = useTranslation();
    const { vpnConnected, connectionInProgress } = useContext(AppContext);
    if (vpnConnected && connectionInProgress === ConnectionInProgress.UNSET)
        return <Divider className={classes.locationDividerCaption} my={0} label={<Text c='green.8' fw={550}>{t('connectedTo')}</Text>} labelPosition='center' />;

    if (connectionInProgress === ConnectionInProgress.UNSET && !vpnConnected)
        return <Divider className={classes.locationDividerCaption} my={0} label={<Text c='gray'>{t('or connect to')}</Text>} labelPosition='center' />;

    return <Space h='1.5rem' />;
}

interface LocationConnectRightButtonProps {
  dropdownOpened: boolean,
  selectedCity: ExitSelectorCity | null,
}

function LocationConnectRightButton({ dropdownOpened, selectedCity }: LocationConnectRightButtonProps) {
    const { t } = useTranslation();
    const theme = useMantineTheme();
    const { vpnConnect, vpnDisconnect, vpnConnected, connectionInProgress, osStatus, showOfflineUI } = useContext(AppContext);
    const inConnectingState = useIsConnecting();
    const buttonText = inConnectingState ? 'Cancel' : (vpnConnected ? 'Disconnect' : 'Connect');
    const btnDisabled = selectedCity === null
      || (dropdownOpened && buttonText === 'Connect')
      || showOfflineUI
      || osStatus.osVpnStatus === NEVPNStatus.Disconnecting
      // to keep the resulting state predictable for the user, disallow cancel when changing locations
      || connectionInProgress === ConnectionInProgress.ChangingLocations;
    // don't want to use color and background props when disabled since they override the disabled styles
    const disconnectVariantProps = !btnDisabled && (isConnecting(connectionInProgress) || vpnConnected) ? theme.other.buttonDisconnectProps : {};
    return (
        <Button miw={130} size='lg' fz='sm' variant='light' disabled={btnDisabled} {...disconnectVariantProps}
            onClick={() => {
                if (vpnConnected || connectionInProgress === ConnectionInProgress.Connecting) {
                    vpnDisconnect();
                } else if (selectedCity !== null) {
                    vpnConnect({
                      city: selectedCity,
                    });
                }
            }}>
            {t(buttonText)}
        </Button>
    );
}

interface CityOptionsProps {
  locations: KeyedSet<Exit>,
  pinnedLocationSet: KeyedSet<PinnedLocation>,
  lastChosenExit: ExitSelector | undefined,
  onExitSelect: (country_code: string, city_code: string) => void
}

interface ItemRightSectionProps {
    exit: Exit,
    hoverKey: string,
    showIconIfPinned?: boolean
}

function CityOptions({ locations, pinnedLocationSet, lastChosenExit, onExitSelect }: CityOptionsProps) {
    const { t } = useTranslation();
    const [hoveredOption, setHoveredKey] = useState<string | null>(null);
    const { appStatus } = useContext(AppContext);
    const connectedExit = appStatus.vpnStatus.connected?.exit;

    if (locations.size === 0) return;

    let lastCity = lastChosenExit && "city" in lastChosenExit && lastChosenExit.city || undefined;

    const ItemRightSection = ({ exit, hoverKey, showIconIfPinned = false }: ItemRightSectionProps) => {
        if (exitCityEquals(connectedExit, exit))
            return <Text size='sm' c='green.8' fw={550}>{t('Connected')}</Text>;

        if (!!hoverKey && hoveredOption === hoverKey)
            return <Text size='sm' c='gray'>{t('clickToConnect')}</Text>;

        if (lastCity?.city_code === exit.city_code && lastCity?.country_code === exit.country_code)
            return <ObscuraChip>{t('lastChosen')}</ObscuraChip>;

        if (showIconIfPinned && pinnedLocationSet.has(exitLocation(exit)))
            return <ThemeIcon variant='transparent' c='dimmed'><BsPinFill /></ThemeIcon>;
    }

    const resetHoverKey = (itemKey: string) => {
        setHoveredKey(value => {
            // avoid any render race condition (confirmed it's possible without this check)
            if (value === itemKey) return null;
            return value;
        })
    }

    const getMouseHoverProps = (itemKey: string) => {
        return { onMouseEnter: () => setHoveredKey(itemKey), onMouseLeave: () => resetHoverKey(itemKey) };
    }

    let orderedLocations = [...locations];
    orderedLocations.sort(exitsSortComparator);

    const result: ReactNode[] = [];

    for (const exit of orderedLocations) {
      if (!pinnedLocationSet.has(exitLocation(exit))) continue;

      if (result.length === 0) {
        result.push(<Text key='pinned-heading' size='sm' c='gray' ml='md' fw={400}><BsPinFill size={11} /> {t('Pinned')}</Text>);
      }

      const key = JSON.stringify(['pinned', exit.country_code, exit.city_code]);
      result.push(
        <Combobox.Option
          className={classes.fixedHoverColor}
          key={key}
          value={key}
          onClick={() => onExitSelect(exit.country_code, exit.city_code)}
          {...getMouseHoverProps(key)}>
          <Group gap='xs' justify='space-between'>
            <Text size='lg'>{getExitCountryFlag(exit)} {exit.city_name}</Text>
            <ItemRightSection exit={exit} hoverKey={key} />
          </Group >
        </Combobox.Option >
      );
    }
    if (result.length > 0) {
      result.push(<Divider key='divider-pinned' my={10} />);
    }

    const insertedContinents = new Set();

    for (const exit of orderedLocations) {
      const continent = getContinent(getExitCountry(exit));
      if (!insertedContinents.has(continent)) {
        if (insertedContinents.size > 0) {
          result.push(<Divider key={`divider-${continent}`} my={10} />);
        }
        insertedContinents.add(continent);
        result.push(<Text key={`continent-${continent}`} size='sm' c='gray' ml='sm' fw={400}>{t(`Continent${continent}`)}</Text>);
      }
      const key = JSON.stringify([exit.country_code, exit.city_code]);

      result.push(
        <Combobox.Option
          className={classes.fixedHoverColor}
          key={key}
          value={key}
          onClick={() => onExitSelect(exit.country_code, exit.city_code)}
          {...getMouseHoverProps(key)}>
          <Group gap='xs' justify='space-between'>
              <Text size='lg'>{getExitCountryFlag(exit)} {exit.city_name}</Text>
              <ItemRightSection exit={exit} hoverKey={key} showIconIfPinned />
          </Group >
        </Combobox.Option >
      );
    }

    return result;
}
