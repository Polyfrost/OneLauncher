import { ParentProps } from 'solid-js';
import WindowFrame from './components/native/WindowFrame';

function App(props: ParentProps) {
    return (
        <main class='bg-bg-primary min-h-screen text-white'>
            <WindowFrame />
            {props.children}
        </main>
    );
}

export default App;
