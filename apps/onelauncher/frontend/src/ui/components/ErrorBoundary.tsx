import type { ParentProps } from 'solid-js';
import { XIcon } from '@untitled-theme/icons-solid';
import { ErrorBoundary as Boundary } from 'solid-js';
import Button from './base/Button';

function ErrorBoundary(props: ParentProps) {
	return (
		<Boundary
			fallback={(err, reset) => (
				<div class="h-full w-full flex flex-1 items-center justify-center">
					<div class="flex flex-row gap-4 border border-danger rounded-md bg-danger/30 p-4">
						<div class="flex flex-1 flex-col gap-2">
							<h2>Error</h2>
							<p>{err.toString()}</p>
						</div>

						<Button buttonStyle="icon" children={<XIcon />} onClick={reset} />
					</div>
				</div>
			)}
		>
			{props.children}
		</Boundary>
	);
}

export default ErrorBoundary;
