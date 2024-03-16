import { JSX, splitProps } from 'solid-js';
import styles from './TextField.module.scss';

type TextFieldProps = {
    iconLeft?: JSX.Element;
    iconRight?: JSX.Element;
} & JSX.InputHTMLAttributes<HTMLInputElement>;

function TextField(props: TextFieldProps) {
    const [fieldProps, rest] = splitProps(props, ['iconLeft', 'iconRight']);

    return (
        <div class={styles.textfield}>
            {fieldProps.iconLeft && <span class={styles.icon}>{fieldProps.iconLeft}</span>}
            <input type="text" {...rest} />
            {fieldProps.iconRight && <span class={styles.icon}>{fieldProps.iconRight}</span>}
        </div>
    );
}

export default TextField;
