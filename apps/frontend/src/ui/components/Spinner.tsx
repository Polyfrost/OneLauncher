import { type JSX, type ParentProps, Suspense, splitProps } from 'solid-js';

export interface SpinnerProps extends JSX.HTMLAttributes<HTMLDivElement> {

};

// https://cssloaders.github.io/
function Spinner(props: SpinnerProps) {
	const [split, rest] = splitProps(props, ['class']);

	return (
		<div
			class={`h-16 w-16 animate-spin animate-duration-1000! inline-block border-t-2 border-t-fg-primary border-r-2 border-r-transparent rounded-1/2 border-solid ${split.class || ''}`}
			{...rest}
		/>
	);
}

export default Spinner;

export type SuspenseSpinnerProps = ParentProps & SpinnerProps;

Spinner.Suspense = function SuspenseSpinner(props: SuspenseSpinnerProps) {
	const [split, rest] = splitProps(props, ['children']);

	return (
		<Suspense fallback={(
			<div class="h-full w-full flex items-center justify-center">
				<Spinner {...rest} />
			</div>
		)}
		>
			{split.children}
		</Suspense>
	);
};
