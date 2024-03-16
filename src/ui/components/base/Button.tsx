import { JSX, ParentProps, mergeProps } from 'solid-js';
import styles from './Button.module.scss';

type ButtonProps = JSX.ButtonHTMLAttributes<HTMLButtonElement> & ParentProps & {
    styleType?: 'primary' | 'secondary' | 'danger',
};

function Button(props: ButtonProps) {
    const merged = mergeProps({ styleType: 'primary' }, props);

    return (
        <button class={`${styles.button} ${styles[`button__${merged.styleType}`]}`} {...props}>
            {props.children}
        </button>
    );
}

export default Button;
