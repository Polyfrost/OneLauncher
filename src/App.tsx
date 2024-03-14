import { invoke } from '@tauri-apps/api/core';

function App() {
    return (
        <div>
            <h1>Test</h1>
            <button onClick={async () => {
                const result = await invoke('plugin:auth|login_msa');
                console.log(result);
            }}>MSA Auth</button>
        </div>
    );
}

export default App;
