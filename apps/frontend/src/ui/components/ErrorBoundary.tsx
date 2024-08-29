import type { ParentProps } from 'solid-js';
// import { ErrorBoundary as Boundary } from 'solid-js';

// TODO: Uncomment code
function ErrorBoundary(props: ParentProps) {
	return (
		<>{props.children}</>
		// <Boundary
		// 	fallback={err => (
		// 		<div class="border border-danger rounded-md bg-danger/20 p-2">
		// 			<h2>An unexpected error has occurred</h2>
		// 			<p>{err.toString()}</p>
		// 		</div>
		// 	)}
		// >
		// 	{props.children}
		// </Boundary>
	);
}

export default ErrorBoundary;
