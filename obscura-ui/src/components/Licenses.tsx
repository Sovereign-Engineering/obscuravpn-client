import { Accordion, Anchor, Code, List, Stack, Title } from '@mantine/core';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { useAsync } from '../common/useAsync.ts';

// Generated by ../../../contrib/licenses.mjs
interface Licenses {
    licenses: License[],
    overview: LicenseSet[],
}

interface LicenseCommon {
    id: string,
    name: string,
}

interface License extends LicenseCommon {
    text: string,
    used_by: UsedBy[],
}

interface LicenseSet extends LicenseCommon {
    count: number,
}

interface UsedBy {
    name: string,
    url: string,
    version: string,
}

export default function Licenses(): React.ReactNode {
    const { t } = useTranslation();
    const { value: licenses } = useAsync<Licenses>({
        // import resolved by vite. See vite.config.js
        load: () => import('$licenses.json'),
    })

    if (!licenses) return;

    return (
        <>
            <Title order={2}>{t('licensesOverview')}</Title>
            <List>
                {licenses.overview.map(license => (
                    <List.Item key={license.id}> {license.name} ({license.count})</List.Item>
                ))}
            </List>
            <Title order={2}>{t('fullLicenseTexts')}</Title>
            <Accordion w='calc(100% - 50px)' variant='separated'>
                {licenses.licenses.map((license, i) => (
                    <Accordion.Item key={license.id} value={`${i}`}>
                        <Accordion.Control id={license.id}>{license.name} - <i>{license.used_by.map(u => u.name).join(', ')}</i></Accordion.Control>
                        <Accordion.Panel>
                            <Stack gap='xs'>
                                <Title order={4}>{t('Packages')}</Title>
                                <List>{license.used_by.map((pkg) => {
                                    return <List.Item key={pkg.name}>
                                        <Anchor href={pkg.url}>
                                            {pkg.name} {pkg.version}
                                        </Anchor>
                                    </List.Item>;
                                })}</List>
                                <Code block style={{ whiteSpace: 'pre-wrap' }}>{license.text}</Code>
                            </Stack>
                        </Accordion.Panel>
                    </Accordion.Item>
                ))}
            </Accordion>
        </>
    );
}
