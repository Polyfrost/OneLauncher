import type { JSX } from 'solid-js';
import { createSignal, createUniqueId, onMount, splitProps } from 'solid-js';
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

	let ref!: HTMLInputElement;

	function validate() {
		if (!split.inputFilter)
			return;

		const value = ref.value;
		const valid = split.inputFilter(value);
		setIsValid(valid);

		if (valid && split.onValidInput)
			split.onValidInput(value);
	}

	function onSubmit() {
		if (isValid() && split.onValidSubmit)
			split.onValidSubmit(ref.value);
	}

	onMount(() => {
		validate();
	});

	return (
		<label for={id} class={`${styles.textfield} ${isValid() ? '' : styles.invalid} ${split.labelClass || ''}`}>
			{split.iconLeft && <span class={styles.icon}>{split.iconLeft}</span>}
			<input
				id={id}
				type="text"
				ref={ref}
				onInput={validate}
				onChange={onSubmit}
				{...rest}
			/>
			{split.iconRight && <span class={styles.icon}>{split.iconRight}</span>}
		</label>
	);
}

type NumberTextFieldProps = TextFieldProps & {
	min?: number;
	max?: number;
};

TextField.Number = (props: NumberTextFieldProps) => {
	const [split, rest] = splitProps(props, ['min', 'max', 'inputFilter']);

	return (
		<TextField
			{...rest}
			inputFilter={(value) => {
				const check_pattern = /^\d*$/.test(value);
				if (!check_pattern)
					return false;

				const check_min = split.min === undefined || Number(value) >= split.min;
				const check_max = split.max === undefined || Number(value) <= split.max;

				if (!check_min || !check_max)
					return false;

				if (split.inputFilter)
					return split.inputFilter(value);

				return true;
			}}
		/>
	);
};

export default TextField;
