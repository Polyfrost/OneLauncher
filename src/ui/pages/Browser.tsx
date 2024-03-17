import { invoke } from '@tauri-apps/api/core';
import Button from '../components/base/Button';

function BrowserPage() {
	function downloadJava() {
		invoke('download_java_test');
	}

	return (
		<div class="flex flex-col gap-y-4">
			<h1>Browser</h1>
			<Button onClick={() => downloadJava()}>Download java</Button>
		</div>
	);
}

export default BrowserPage;
