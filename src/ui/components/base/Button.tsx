import { JSX, ParentProps, mergeProps } from 'solid-js';
import styles from './Button.module.scss';

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & ParentProps & {
    size?: 'sm' | 'md' | 'lg',
    style?: 'primary' | 'secondary' | 'danger', // TODO
};

function Button(props: ButtonProps) {
    const merged = mergeProps({ size: 'md' }, props);

    return (
        <button class={`${styles.button} ${styles[`size-${merged.size}`]}`} {...props}>
            {props.children}
        </button>
    );
}

export default Button;
