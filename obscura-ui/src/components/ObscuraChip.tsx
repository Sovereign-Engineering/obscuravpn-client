import { Text } from '@mantine/core';
import React from 'react';

import commonClasses from '../common/common.module.css';

/**
 * Unlike MantineChip, ObscuraChip is not a clickable element
 */
export default function ObscuraChip({ children }) {
    return <Text size='sm' c='teal' px={8} py={2} className={commonClasses.chip}>{children}</Text>;
}
