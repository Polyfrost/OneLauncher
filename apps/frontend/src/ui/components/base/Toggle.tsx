import { type Accessor, type JSX, createEffect, createSignal, splitProps } from 'solid-js';

type ToggleProps = JSX.HTMLAttributes<HTMLDivElement> & {
	checked?: Accessor<boolean>;
	onChecked?: (checked: boolean) => void;
};

function Toggle(props: ToggleProps) {
	// eslint-disable-next-line solid/reactivity -- ok
	const [checked, setChecked] = createSignal(props.checked?.());
	const [split, rest] = splitProps(props, ['class', 'checked', 'onChecked', 'text']);

	// eslint-disable-next-line solid/reactivity -- i hate mylife
	if (props.checked)
		createEffect(() => {
			setChecked(props.checked!());
		});

	function toggle() {
		const newValue = !checked();
		setChecked(newValue);
		props.onChecked?.(newValue);
	}

	return (
		<div
			onClick={() => toggle()}
			class={`w-[40px] h-[22px] p-3 flex flex-row relative rounded-full transition-colors overflow-hidden ${checked() ? 'bg-brand' : 'bg-gray-10'}${` ${split.class}` || ''}`}
			{...rest}
		>
			<div class={`w-[16px] h-[16px] rounded-full mx-1 left-0 top-1/2 -translate-y-1/2 transition-transform bg-white absolute ${checked() ? 'translate-x-full' : 'translate-x-0'}`} />
		</div>
	);
}

export default Toggle;
