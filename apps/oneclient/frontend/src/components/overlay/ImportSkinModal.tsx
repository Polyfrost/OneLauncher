import { Overlay } from '@/components';
import { Button, TextField } from '@onelauncher/common/components';
import { useState } from 'react';

export function ImportSkinModal({ importFromURL, importFromUsername }: { importFromURL: (url: string) => void; importFromUsername: (username: string) => void }) {
	const [input, setInput] = useState<string>('');
	return (
		<Overlay.Dialog>
			<Overlay.Title>Import</Overlay.Title>
			<TextField className="w-full" onChange={e => setInput(e.target.value)} />

			<div className="flex flex-row gap-4 h-1/2 w-full">
				<Button
					className="w-1/2"
					color="primary"
					onClick={() => importFromUsername(input)}
					size="normal"
					slot="close"
				>
					From Username
				</Button>
				<Button
					className="w-1/2"
					color="primary"
					onClick={() => importFromURL(input)}
					size="normal"
					slot="close"
				>
					From URL
				</Button>
			</div>
		</Overlay.Dialog>
	);
}
