import type { JSX, ParentProps } from 'solid-js';
import { mergeProps, splitProps } from 'solid-js';
import styles from './Button.module.scss';

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & ParentProps & {
	iconLeft?: JSX.Element;
	iconRight?: JSX.Element;
	styleType?: 'primary' | 'secondary' | 'danger' | 'icon' | 'ghost';
};

function Button(props: ButtonProps) {
	const [buttonProps, rest] = splitProps(props, ['iconLeft', 'iconRight', 'styleType', 'class']);
	const merged = mergeProps({ styleType: 'primary' }, buttonProps);

	return (
		<button class={`${styles.button} ${styles[`button__${merged.styleType}`]} ${buttonProps.class || ''}`} {...rest}>
			{merged.iconLeft && <span class={styles.icon}>{merged.iconLeft}</span>}
			{props.children}
			{merged.iconRight && <span class={styles.icon}>{merged.iconRight}</span>}
		</button>
	);
}

export default Button;
