import type { Accessor, JSX, ParentProps } from 'solid-js';
import { createEffect, createSignal, mergeProps, on, splitProps, untrack } from 'solid-js';
import styles from './Button.module.scss';

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & ParentProps & {
	iconLeft?: JSX.Element;
	iconRight?: JSX.Element;
	buttonStyle?: 'primary' | 'secondary' | 'danger' | 'icon' | 'iconPrimary' | 'iconSecondary' | 'iconDanger' | 'ghost';
	large?: boolean;
};

function Button(props: ButtonProps) {
	const [buttonProps, rest] = splitProps(props, ['iconLeft', 'iconRight', 'buttonStyle', 'class', 'large']);
	const merged = mergeProps({ buttonStyle: 'primary', large: false }, buttonProps);

	return (
		<button class={`${styles.button} ${styles[`button__${merged.buttonStyle}`]} ${merged.large ? styles.large : ''} ${buttonProps.class || ''}`} {...rest}>
			{merged.iconLeft && <span class={styles.icon}>{merged.iconLeft}</span>}
			{props.children}
			{merged.iconRight && <span class={styles.icon}>{merged.iconRight}</span>}
		</button>
	);
}

type ButtonToggleProps = ButtonProps & {
	checked?: Accessor<boolean>;
	onChecked?: (checked: boolean) => any;
};

Button.Toggle = (props: ButtonToggleProps) => {
	const [split, rest] = splitProps(props, ['checked', 'onChecked', 'onClick']);
	const [checked, setChecked] = createSignal(untrack(() => split.checked?.()) || false);

	createEffect(on(() => split.checked?.(), newValue => setChecked(newValue || false)));

	function toggle() {
		const newValue = !checked();
		setChecked(newValue);
		props.onChecked?.(newValue);
	}

	return (
		<Button
			aria-checked={checked()}
			onClick={(e) => {
				toggle();
				if (typeof split.onClick === 'function')
					split.onClick(e);
			}}
			{...rest}
		/>
	);
};

export default Button;
