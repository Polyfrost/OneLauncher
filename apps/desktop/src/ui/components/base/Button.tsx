import type { JSX, ParentProps } from 'solid-js';
import { mergeProps, splitProps } from 'solid-js';
import styles from './Button.module.scss';

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & ParentProps & {
	iconLeft?: JSX.Element;
	iconRight?: JSX.Element;
	buttonStyle?: 'primary' | 'secondary' | 'danger' | 'icon' | 'iconSecondary' | 'ghost';
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

export default Button;
