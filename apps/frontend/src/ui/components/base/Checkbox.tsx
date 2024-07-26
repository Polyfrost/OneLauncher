import { type JSX, createSignal, splitProps } from 'solid-js';
import { CheckIcon } from '@untitled-theme/icons-solid';
import styles from './Checkbox.module.scss';

type CheckboxProps = JSX.HTMLAttributes<HTMLDivElement> & {
	defaultChecked?: boolean;
	onChecked?: (checked: boolean) => void;
};

function Checkbox(props: CheckboxProps) {
	const [checked, setChecked] = createSignal(props.defaultChecked || false);
	const [split, rest] = splitProps(props, ['class', 'defaultChecked', 'onChecked', 'text']);

	function toggle() {
		const newValue = !checked();
		setChecked(newValue);
		props.onChecked?.(newValue);
	}

	return (
		<div
			onClick={() => toggle()}
			class={`${styles.checkbox} ${` ${split.class}` ?? ''}`}
			{...rest}
		>
			<div class={`${styles.box} ${checked() ? styles.checked : ''}`}>
				<CheckIcon />
			</div>
			{props.children}
		</div>
	);
}

export default Checkbox;
