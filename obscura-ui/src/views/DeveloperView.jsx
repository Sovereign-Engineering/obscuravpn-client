import { ActionIcon, Button, Group, JsonInput, Stack, Text, TextInput, Title } from '@mantine/core';
import { useInterval } from '@mantine/hooks';
import { notifications } from '@mantine/notifications';
import { relaunch } from '@tauri-apps/api/process';
import { useContext, useEffect, useRef, useState } from 'react';
import { IoLogOutOutline } from 'react-icons/io5';
import { IS_WK_WEB_VIEW } from '../common/utils';
import { AppContext } from '../common/appContext';
import * as commands from '../tauri/commands';
import { RUNNING_IN_TAURI } from '../tauri/SystemProvider';
import DevSendCommand from '../components/DevSendCommand';
import Cookies from 'js-cookie';
import DevSetApiUrl from '../components/DevSetApiUrl';

export default function DeveloperViewer() {
    const { vpnConnected, connectionInProgress, appStatus, osStatus } = useContext(AppContext);
    const [trafficStats, setTrafficStats] = useState({});
    const [exitServers, setExitServers] = useState([]);
    const cookieToDeleteRef = useRef(null);

    const trafficStatsInterval = useInterval(async () => {
        setTrafficStats(await commands.getTrafficStats());
    }, 1000);

    useEffect(() => {
        (async () => { setExitServers(await commands.getExitServers()); })();
        trafficStatsInterval.start();
        return () => {
            trafficStatsInterval.stop();
        };
    }, []);

    return <Stack p={20}>
        <Title order={3}>Developer View</Title>
        <Title order={4}>Current Status</Title>
        <JsonInput value={JSON.stringify(appStatus, null, 4)} contentEditable={false} rows={11} />
        <JsonInput value={JSON.stringify(osStatus, null, 4)} contentEditable={false} rows={4} />
        <Title order={4}>React variables</Title>
        <Text>vpn is connected: <b>{vpnConnected ? 'Yes' : 'No'}</b></Text>
        <Text>connection in progress: <b>{connectionInProgress ?? 'No'}</b></Text>
        {IS_WK_WEB_VIEW && <><Button title='Preferences such as whether the user has been onboarded or if the app has tried to register as a login item' onClick={() => commands.developerResetUserDefaults().then(() => notifications.show({ title: 'Successfully Removed UserDefault Keys', color: 'green' }))}>Reset app UserDefaults</Button></>}
        {RUNNING_IN_TAURI && <Button onClick={relaunch}>Relaunch</Button>}
        <DevSetApiUrl />
        <Title order={4}>Traffic Stats</Title>
        <Text>Since this is cumulative, to get the average bandwidth speed, you must do a slope calculation between the time of two captures (recommended gap of 500ms to 1000ms). See code in the <code>apple/client/StatusItem</code> directory for reference</Text>
        <JsonInput value={JSON.stringify(trafficStats, null, 4)} contentEditable={false} rows={6} />
        <Title order={4}>Exit Servers</Title>
        <JsonInput value={JSON.stringify(exitServers, null, 4)} contentEditable={false} rows={4} />
        <Group gap={10}>
            <ActionIcon title='logout' variant='default' onClick={() => commands.logout().catch(e => notifications.show({ title: 'logoutFailed', message: e.type === 'logoutFailed' ? t('pleaseReportError') : '' }))
            } size={30}>
                <IoLogOutOutline />
            </ActionIcon>
            <Text>Logout</Text>
        </Group>
        <DevSendCommand />
        <Button onClick={() => commands.setInNewAccountFlow(true)}>setInNewAccountFlow</Button>
        <Title order={4}>Cookies</Title>
        <Text>{JSON.stringify(Cookies.get(), null, 4)}</Text>
        <Group>
        <TextInput ref={cookieToDeleteRef} placeholder='cookieName' />
        <Button onClick={() => Cookies.remove(cookieToDeleteRef.current.value)}>Delete Cookie</Button>
        </Group>
    </Stack>;
}
