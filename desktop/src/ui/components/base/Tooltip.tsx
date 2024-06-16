import { type JSX, splitProps } from 'solid-js';
import styles from './Tooltip.module.scss';

export type TooltipProps = JSX.HTMLAttributes<HTMLDivElement> & {
	text: string;
};

function Tooltip(props: TooltipProps) {
	const [split, rest] = splitProps(props, ['class']);

	return (
		<div data-text={props.text} class={`${styles.tooltip} ${split.class}`} {...rest} />
	);
}

export default Tooltip;
