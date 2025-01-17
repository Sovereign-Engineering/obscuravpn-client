import React from 'react';

if (process.env.NODE_ENV === 'development') {
  const whyDidYouRender = (await import('@welldone-software/why-did-you-render')).default;
  whyDidYouRender(React, {
    // use `Component.whyDidYouRender = true` instead
    trackAllPureComponents: false,
  });
}
