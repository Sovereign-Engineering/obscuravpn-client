import { Center, Group, Loader, Text } from '@mantine/core';
export default function ({ text = '' }) {
    // filler
    return <Center style={{ height: '100vh', width: '100vw' }}>
        {/* TODO: rotating logo */}
        <Group>
            {text.length > 0 && <Text>{text}</Text>}
            <Loader size='xl' type='bars' />
        </Group>
    </Center>;
}
