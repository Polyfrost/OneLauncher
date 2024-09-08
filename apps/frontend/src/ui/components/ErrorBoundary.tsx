import { ErrorBoundary as Boundary, Match, Switch } from 'solid-js';
import type { ParentProps } from 'solid-js';

function ErrorBoundary(props: ParentProps) {
	return (
		<Switch>
			<Match when={import.meta.env.DEV !== true}>
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
			</Match>
			<Match when>
				{props.children}
			</Match>
		</Switch>
	);
}

export default ErrorBoundary;
