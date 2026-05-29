import { ActionIcon, Affix, ScrollArea, Transition } from '@mantine/core';
import { PropsWithChildren, RefObject, useRef, useState } from 'react';
import { IoArrowUp } from 'react-icons/io5';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';
import classes from './ScrollableView.module.css';

export function ScrollableView({ children }: PropsWithChildren) {
  const viewport = useRef<HTMLDivElement>(null);
  const [scrollY, setScrollY] = useState(0);

  return (
    <ScrollArea h='100vh' type='always' scrollbarSize={IS_HANDHELD_DEVICE ? 2 : 12} classNames={classes} viewportRef={viewport} onScrollPositionChange={({ y }) => setScrollY(y)}>
      {children}
      <ScrollToTop scrollY={scrollY} viewport={viewport} />
    </ScrollArea>
  );
}

function ScrollToTop({ scrollY, viewport }: { scrollY: number, viewport: RefObject<HTMLDivElement | null> }) {
  return (
    <Affix position={{ bottom: 'calc(20px + var(--safe-area-inset-bottom, env(safe-area-inset-bottom)))', right: 'calc(20px + var(--safe-area-inset-right, env(safe-area-inset-right)))' }}>
      <Transition transition='slide-up' mounted={scrollY > 50}>
        {transitionStyles =>
          <ActionIcon style={transitionStyles} size='lg' variant='gradient'
            onClick={() => viewport.current?.scrollTo({ top: 0, behavior: 'smooth' })}>
            <IoArrowUp size={25} />
          </ActionIcon>
        }
      </Transition>
    </Affix>
  );
}
