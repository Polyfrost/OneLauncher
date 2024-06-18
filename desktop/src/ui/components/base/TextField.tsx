import type { JSX } from 'solid-js';
import { createEffect, createSignal, createUniqueId, splitProps } from 'solid-js';
import styles from './TextField.module.scss';

type TextFieldProps = {
	iconLeft?: JSX.Element;
	iconRight?: JSX.Element;
	inputFilter?: (value: string) => boolean;
	onValidInput?: (value: string) => any;
	onValidSubmit?: (value: string) => any;
} & JSX.InputHTMLAttributes<HTMLInputElement>;

function TextField(props: TextFieldProps) {
	const [fieldProps, rest] = splitProps(props, ['iconLeft', 'iconRight', 'inputFilter', 'onValidInput', 'onValidSubmit']);
	const [isValid, setIsValid] = createSignal(true);
	const id = createUniqueId();

	function validate(e: Event & { currentTarget: HTMLInputElement }) {
		if (!fieldProps.inputFilter)
			return;

		const value = e.currentTarget.value;
		const valid = fieldProps.inputFilter(value);
		setIsValid(valid);

		if (valid && fieldProps.onValidInput)
			fieldProps.onValidInput(value);
	}

	function onSubmit(e: Event & { currentTarget: HTMLInputElement }) {
		if (isValid() && fieldProps.onValidSubmit)
			fieldProps.onValidSubmit(e.currentTarget.value);
	}

	return (
		<label for={id} class={`${styles.textfield} ${isValid() ? {} : styles.invalid}`}>
			{fieldProps.iconLeft && <span class={styles.icon}>{fieldProps.iconLeft}</span>}
			<input
				id={id}
				type="text"
				onInput={validate}
				onChange={onSubmit}
				{...rest}
			/>
			{fieldProps.iconRight && <span class={styles.icon}>{fieldProps.iconRight}</span>}
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
