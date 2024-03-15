import auth from './bridge/auth';

function App() {
    return (
        <button onClick={() => auth.loginMicrosoft()}>loginMicrosoft</button>
    );
}

export default App;
