import { ActionIcon, Affix, Transition, useComputedColorScheme } from '@mantine/core';
import { useEffect, useState } from 'react';
import { IoArrowUp } from 'react-icons/io5';

export function ScrollToTop({ scroller, bottom = 10 }: { scroller: HTMLElement, bottom: number }) {
  const [scrollY, setScrollY] = useState<number>();
  const colorScheme = useComputedColorScheme();

  const handleScroll = () => {
    setScrollY(scroller.scrollTop);
  };

  useEffect(() => {
    if (scroller !== null) {
      scroller.addEventListener('scroll', handleScroll, { passive: true });
      return () => {
        scroller.removeEventListener('scroll', handleScroll);
      };
    }
  }, [scroller]);

  return <Affix position={{ bottom, right: 20 }}>
    <Transition transition='slide-up' mounted={scrollY !== undefined && scrollY > 0}>
      {transitionStyles =>
        <ActionIcon style={transitionStyles} size='lg' variant={colorScheme === 'dark' ? 'light' : 'filled'}
          onClick={() => scroller.scrollTo({ top: 0, behavior: 'smooth' })}>
          <IoArrowUp size={25} />
        </ActionIcon>
      }
    </Transition>
  </Affix>;
}
