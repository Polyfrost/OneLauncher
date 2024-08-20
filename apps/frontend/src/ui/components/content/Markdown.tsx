import type { ParentProps } from 'solid-js';
import { renderHighlightedString } from '@onelauncher/client';
import styles from './Markdown.module.scss';
import type { Package } from '~bindings';

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
