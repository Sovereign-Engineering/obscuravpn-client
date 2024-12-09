import { FallbackProps } from 'react-error-boundary';
import { normalizeError } from '../../common/utils';

// NOTE: this component CANNOT USE HOOKS
export default function FallbackAppRender({ error, resetErrorBoundary }: FallbackProps) {
  // call resetErrorBoundary() to reset the error boundary and retry the render.
  return (
    <div role='alert' style={{ margin: 10 }}>
      <h2>Fatal Error While Rendering</h2>
      <p>Click the Obscura VPN status icon in the status menu (the area with the battery icon), and then click "Create Debugging Archive." A finder window should spawn with the zip file selected. Please send this file to us as well a screenshot of the following error.</p>
      <h3>What went wrong</h3>
      <pre style={{ color: 'red', fontWeight: 'bold', whiteSpace: 'break-spaces', marginBottom: '1.5em', marginLeft: '1.5em' }}>{normalizeError(error).message}</pre>
      <button style={{ background: '#ff5f25', borderRadius: 5 }} onClick={resetErrorBoundary}>Refresh</button>
      <p>After creating a debugging archive, you can try to get back to a usable state by pressing the "Refresh" button above. If this message is seen immediately after reload, an app update may also be needed.</p>
    </div>
  );
}
