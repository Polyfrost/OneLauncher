import type { ParentProps } from 'solid-js';
import { renderHighlightedString } from '@onelauncher/client';
import styles from './Markdown.module.scss';
import usePromptOpener from '~ui/hooks/usePromptOpener';

export type MarkdownProps = ParentProps & {
	body: string;
};

function Markdown(props: MarkdownProps) {
	const promptOpen = usePromptOpener();

	return (
		<div
			class={styles.markdown}
			// eslint-disable-next-line solid/no-innerhtml -- Should be sanitised properly
			innerHTML={renderHighlightedString(props.body)}
			onContextMenu={(e) => {
				e.stopImmediatePropagation();
				e.preventDefault();
			}}
			onClick={(e) => {
				if (e.target.nodeName === 'A') {
					e.stopImmediatePropagation();
					e.preventDefault();

					const target = e.target as HTMLAnchorElement;
					if (target.href.startsWith('http'))
						promptOpen(target.href);
				}
			}}
		/>
	);
}

export default Markdown;
