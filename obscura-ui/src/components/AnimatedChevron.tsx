import React from 'react';
import { BsChevronDown } from 'react-icons/bs';

export default function AnimatedChevron({ rotated }: { rotated: Boolean }) {
    return (
        <BsChevronDown
            size={16}
            style={{
                transform: rotated ? 'rotate(-180deg)' : undefined,
                transition: 'transform 200ms ease-in-out'
            }}
        />
    );
}
