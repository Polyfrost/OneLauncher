import type React from 'react';
import type { HTMLProps } from 'react';
import { bindings } from '@/main';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { LinkExternal01Icon } from '@untitled-theme/icons-react';
import { useState } from 'react';
import { twMerge } from 'tailwind-merge';
import { Overlay } from './overlay';

export interface ExternalLinkProps extends HTMLProps<HTMLAnchorElement> {
	href?: string | undefined;
	children?: React.ReactNode;
	includeIcon?: boolean | undefined;
	showModal?: boolean | undefined;
}

export function ExternalLink({
	href,
	children,
	includeIcon,
	showModal = true,
	className,
	...rest
}: ExternalLinkProps) {
	const [isOpen, setIsOpen] = useState(false);

	// done like this so that "image" links don't have an icon
	const withIcon = !!(includeIcon ?? typeof children === 'string');

	return (
		<Overlay.Trigger isOpen={isOpen} onOpenChange={setIsOpen}>
			<a
				className={twMerge('text-fg-primary hover:opacity-80 underline', className)}
				href={href}
				onClick={(e) => {
					e.preventDefault();
					if (!href)
						return;

					if (showModal)
						setIsOpen(true);
					else
						bindings.core.open(href);
				}}
				role="button"
				{...rest}
			>
				{children}
				{withIcon && <LinkExternal01Icon className="inline w-3 ml-0.5 h-3" />}
			</a>

			<Overlay>
				<Overlay.Dialog>
					<Overlay.Title>External Link</Overlay.Title>
					<p>Are you sure you want to open this link in your browser?</p>

					<code className="w-full text-center bg-component-bg py-2 rounded-md overflow-auto max-w-100 text-nowrap">{href}</code>

					<Overlay.Buttons
						buttons={[
							{
								color: 'secondary',
								children: 'Copy Link',
								onClick: () => {
									if (href)
										writeText(href);

									setIsOpen(false);
								},
							},
							{
								color: 'primary',
								children: 'Open Link',
								onClick: () => {
									if (href)
										bindings.core.open(href);

									setIsOpen(false);
								},
							},
						]}
					/>
				</Overlay.Dialog>
			</Overlay>
		</Overlay.Trigger>
	);
}
