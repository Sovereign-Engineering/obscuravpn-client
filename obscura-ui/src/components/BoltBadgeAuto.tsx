import React from 'react';
import SvgFile from '../res/bolt.badge.automatic.fill.svg?react';

export default function BoltBadgeAuto({ height = '1.25em', fill = 'white' }) {
    return <SvgFile fill={fill} height={height} />
}
