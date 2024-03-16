import { JSX, ParentProps } from 'solid-js';
import styles from './Tag.module.scss';

type TagProps = ParentProps & {
    iconLeft?: JSX.Element,
    iconRight?: JSX.Element,
};

function Tag(props: TagProps) {
    return (
        <span class={styles.tag}>
            {props.iconLeft && <span class={styles.icon}>{props.iconLeft}</span>}
            {props.children && props.children}
            {props.iconRight && <span class={styles.icon}>{props.iconRight}</span>}
        </span>
    );
}

export default Tag;
