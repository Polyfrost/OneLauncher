import type { ParentProps } from 'solid-js';
import { ErrorBoundary as Boundary } from 'solid-js';
import { useDebugState } from '@onelauncher/client';
import Button from './base/Button';
import Popup from './overlay/Popup';

// TODO: Uncomment code
function ErrorBoundary(props: ParentProps) {
	return (
		<Boundary
			fallback={err => (
				<div class="border border-danger rounded-md bg-danger/20 p-2">
					<h2>An unexpected error has occurred</h2>
					<p>{err.toString()}</p>
				</div>
			)}
		>
			{props.children}
		</Boundary>
	);
}

export default ErrorBoundary;
