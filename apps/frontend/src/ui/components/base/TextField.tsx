import type { JSX } from 'solid-js';
import { createSignal, createUniqueId, splitProps } from 'solid-js';
import styles from './TextField.module.scss';

type TextFieldProps = {
	iconLeft?: JSX.Element;
	iconRight?: JSX.Element;
	inputFilter?: (value: string) => boolean;
	onValidInput?: (value: string) => any;
	onValidSubmit?: (value: string) => any;
	labelClass?: string;
} & JSX.InputHTMLAttributes<HTMLInputElement>;

function TextField(props: TextFieldProps) {
	const [split, rest] = splitProps(props, ['iconLeft', 'iconRight', 'inputFilter', 'onValidInput', 'onValidSubmit', 'labelClass']);
	const [isValid, setIsValid] = createSignal(true);
	const id = createUniqueId();

	function validate(e: Event & { currentTarget: HTMLInputElement }) {
		if (!split.inputFilter)
			return;

		const value = e.currentTarget.value;
		const valid = split.inputFilter(value);
		setIsValid(valid);

		if (valid && split.onValidInput)
			split.onValidInput(value);
	}

	function onSubmit(e: Event & { currentTarget: HTMLInputElement }) {
		if (isValid() && split.onValidSubmit)
			split.onValidSubmit(e.currentTarget.value);
	}

	return (
		<label for={id} class={`${styles.textfield} ${isValid() ? '' : styles.invalid} ${split.labelClass || ''}`}>
			{split.iconLeft && <span class={styles.icon}>{split.iconLeft}</span>}
			<input
				id={id}
				type="text"
				onInput={validate}
				onChange={onSubmit}
				{...rest}
			/>
			{split.iconRight && <span class={styles.icon}>{split.iconRight}</span>}
		</label>
	);
}

TextField.Number = (props: TextFieldProps) => (
	<TextField
		inputFilter={value => /^\d*$/.test(value)}
		{...props}
	/>
);

export default TextField;
