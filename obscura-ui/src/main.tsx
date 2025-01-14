import './wdyr';
import React from 'react';
import { createRoot } from 'react-dom/client';
import { ErrorBoundary } from 'react-error-boundary';
import App from './App';
import Providers from './Providers';
import { logReactError } from './bridge/SystemProvider';
import './translations/i18n'; // for internationalization (translations)
import { FallbackAppRender } from './views';

const root = createRoot(document.getElementById('root')!);
root.render(
  <React.StrictMode>
    <Providers>
      <ErrorBoundary
        FallbackComponent={FallbackAppRender}
        // Reset the state of your app so the error doesn't happen again
        onReset={details => {
          location.pathname = '/';
        }}
        onError={logReactError}>
        <App />
      </ErrorBoundary>
    </Providers>
  </React.StrictMode>
);
