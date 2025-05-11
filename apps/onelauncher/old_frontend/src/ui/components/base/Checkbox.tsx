import { CheckIcon } from '@untitled-theme/icons-solid';
import { createSignal, type JSX, Show, splitProps, untrack } from 'solid-js';
import styles from './Checkbox.module.scss';

type CheckboxProps = JSX.HTMLAttributes<HTMLDivElement> & {
	defaultChecked?: boolean;
	onChecked?: (checked: boolean) => void;
};

function Checkbox(props: CheckboxProps) {
	const [checked, setChecked] = createSignal(props.defaultChecked || false);
	const [split, rest] = splitProps(props, ['class', 'defaultChecked', 'onChecked', 'text']);

	function toggle(e: MouseEvent) {
		e.preventDefault();
		e.stopPropagation();

		const newValue = !untrack(checked);
		setChecked(newValue);
		props.onChecked?.(newValue);
	}

	return (
		<div
			class={`${styles.checkbox} ${split.class ?? ''}`}
			onClick={toggle}
			{...rest}
		>
			<div class={`${styles.box} ${checked() ? styles.checked : ''}`}>
				<Show when={checked()}>
					<CheckIcon />
				</Show>
			</div>
			{props.children}
		</div>
	);
}

export default Checkbox;
