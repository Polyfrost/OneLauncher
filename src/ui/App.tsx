import { ParentProps } from 'solid-js';
import { Transition } from 'solid-transition-group';
import WindowFrame from './components/native/WindowFrame';
import Navbar from './components/Navbar';

function App(props: ParentProps) {
    return (
        <main class='flex flex-col bg-primary min-h-screen text-white'>
            <WindowFrame />
            <Navbar />
            <div class='flex flex-col flex-1 *:flex-1 overflow-x-hidden px-8'>
                { /* eslint-disable-next-line @typescript-eslint/no-use-before-define */ }
                <AnimatedRoutes>
                    {props.children}
                </AnimatedRoutes>
            </div>
        </main>
    );
}

export default App;

function AnimatedRoutes(props: ParentProps) {
    const keyframesEnter = [
        {
            opacity: 0,
            transform: 'translateX(-100px)',
        },
        {
            opacity: 1,
            transform: 'translateX(0px)',
        },
    ];

    const keyframesExit = [
        {
            opacity: 1,
            transform: 'translateX(0px)',
        },
        {
            opacity: 0,
            transform: 'translateX(100px)',
        },
    ];

    const properties: KeyframeAnimationOptions = {
        duration: 100,
        easing: 'cubic-bezier(0.22, 1, 0.36, 1)',
    };

    return (
        <Transition
            mode='outin'
            onEnter={(element, done) => {
                const animation = element.animate(
                    keyframesEnter,
                    properties,
                );

                animation.onfinish = done;
            }}
            onExit={(element, done) => {
                const animation = element.animate(
                    keyframesExit,
                    properties,
                );

                animation.onfinish = done;
            }}
        >
            {props.children}
        </Transition>
    );
}
