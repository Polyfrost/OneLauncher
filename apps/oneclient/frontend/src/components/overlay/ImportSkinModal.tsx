import { Button, TextField } from '@onelauncher/common/components';
import { Overlay } from './Overlay';

export function ImportSkinModal() {
	return (
		<Overlay.Dialog>
			<Overlay.Title>Import</Overlay.Title>
			<TextField className="w-full" />

			<div className='flex flex-row gap-4 h-1/2 w-full'>
				<Button className="w-1/2" color="primary" size="normal" slot="close">
					From Username
				</Button>
				<Button className="w-1/2" color="primary" size="normal" slot="close">
					From URL
				</Button>
			</div>
		</Overlay.Dialog>
	);
}
