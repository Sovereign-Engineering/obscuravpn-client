import { ActionIcon, Affix, Transition } from '@mantine/core';
import { useEffect, useState } from 'react';
import { IoArrowUp } from 'react-icons/io5';

export function ScrollToTop({ scroller, bottom = 10 }: { scroller: HTMLElement | null, bottom: number }) {
  const [scrollY, setScrollY] = useState<number>();

  const handleScroll = () => {
    if (scroller !== null) {
      setScrollY(scroller.scrollTop);
    }
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
        <ActionIcon style={transitionStyles} size='lg' variant='gradient'
          onClick={() => scroller?.scrollTo({ top: 0, behavior: 'smooth' })}>
          <IoArrowUp size={25} />
        </ActionIcon>
      }
    </Transition>
  </Affix>;
}
