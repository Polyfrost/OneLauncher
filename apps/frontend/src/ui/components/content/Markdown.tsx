import type { ParentProps } from 'solid-js';
import { renderHighlightedString } from '@onelauncher/client';
import styles from './Markdown.module.scss';

export type MarkdownProps = ParentProps & {
	body: string;
};

function Markdown(props: MarkdownProps) {
	return (
		<div
			class={styles.markdown}
			// eslint-disable-next-line solid/no-innerhtml -- Should be sanitised properly
			innerHTML={renderHighlightedString(props.body)}
			onClick={(e) => {
				e.stopImmediatePropagation();
				e.preventDefault();
			}}
		/>
	);
}

export default Markdown;
