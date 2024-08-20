import type { ParentProps } from 'solid-js';
import { renderHighlightedString } from '@onelauncher/client';
import type { Package } from '@onelauncher/client/bindings';
import styles from './Markdown.module.scss';

export type MarkdownProps = ParentProps & {
	package: Package;
};

function Markdown(props: MarkdownProps) {
	return (
		<div class={styles.markdown}>
			{renderHighlightedString(props.package.meta.body)}
		</div>
	);
}

export default Markdown;
