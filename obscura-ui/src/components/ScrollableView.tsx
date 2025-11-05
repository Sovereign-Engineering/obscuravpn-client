import { ActionIcon, Affix, ScrollArea, Transition } from '@mantine/core';
import { useWindowScroll } from '@mantine/hooks';
import { PropsWithChildren, useRef } from 'react';
import { IoArrowUp } from 'react-icons/io5';
import classes from './ScrollableView.module.css';
import { IS_HANDHELD_DEVICE } from '../bridge/SystemProvider';

export function ScrollableView({ children }: PropsWithChildren) {
  const viewport = useRef<HTMLDivElement>(null);

  return (
    <ScrollArea type='always' scrollbarSize={IS_HANDHELD_DEVICE ? 2 : 12} classNames={classes} viewportRef={viewport}>
      {children}
      <ScrollToTop />
    </ScrollArea>
  );
}

function ScrollToTop() {
  const [scroll, scrollTo] = useWindowScroll();

  return (
    <Affix position={{ bottom: 20, right: 20 }}>
      <Transition transition='slide-up' mounted={scroll.y > 50}>
        {transitionStyles =>
          <ActionIcon style={transitionStyles} size='lg' variant='gradient'
            onClick={() => scrollTo!({ y: 0 })}>
            <IoArrowUp size={25} />
          </ActionIcon>
        }
      </Transition>
    </Affix>
  );
}
