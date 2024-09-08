import { type JSX, splitProps } from 'solid-js';
import styles from './Tooltip.module.scss';

export type TooltipProps = JSX.HTMLAttributes<HTMLDivElement> & {
	text: string;
};

function Tooltip(props: TooltipProps) {
	const [split, rest] = splitProps(props, ['class']);

	return (
		<div class={`${styles.tooltip} ${split.class || ''}`} data-text={props.text} {...rest} />
	);
}

export default Tooltip;
