import { LinkExternal01Icon } from '@untitled-theme/icons-solid';
import usePromptOpener from '~ui/hooks/usePromptOpener';
import { type JSX, splitProps } from 'solid-js';
import styles from './Link.module.scss';

export type LinkProps = JSX.IntrinsicElements['a'] & {
	skipPrompt?: boolean;
	includeIcon?: boolean;
};

function Link(props: LinkProps) {
	const [split, rest] = splitProps(props, ['skipPrompt', 'class', 'href', 'onClick', 'children']);
	const open = usePromptOpener();

	return (
		<button
			{...rest as JSX.ButtonHTMLAttributes<HTMLButtonElement>}
			children={(
				<div class="flex flex-row gap-x-1">
					{props.children}
					{props.includeIcon === true && (
						<LinkExternal01Icon class="h-3 min-h-3 min-w-3 w-3" />
					)}
				</div>
			)}
			class={`${styles.link} ${split.class || ''}`}
			onClick={(e) => {
				open(split.href, !!split.skipPrompt);
				if (typeof split.onClick === 'function')
					split.onClick(e as any);
			}}
		/>
	);
}

export default Link;
