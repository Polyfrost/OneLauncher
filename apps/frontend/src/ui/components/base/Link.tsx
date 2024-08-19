import { type JSX, splitProps } from 'solid-js';
import styles from './Link.module.scss';
import usePromptOpener from '~ui/hooks/usePromptOpener';

export type LinkProps = JSX.IntrinsicElements['a'] & {
	prompt?: boolean;
};

function Link(props: LinkProps) {
	const [split, rest] = splitProps(props, ['prompt', 'class', 'href', 'onClick']);
	const open = usePromptOpener();

	function onClick() {
		open(split.href, !!split.prompt);

		// @ts-expect-error -- This should be valid
		props.onClick?.();
	}

	return (
		<button
			{...rest as any}
			class={`${styles.link} ${split.class || ''}`}
			onClick={onClick}
		/>
	);
}

export default Link;
