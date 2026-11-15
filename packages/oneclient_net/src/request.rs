#[derive(Debug)]
pub struct HttpRequest {
    pub request: reqwest::Request,
    pub options: RequestOptions,
}

impl From<reqwest::Request> for HttpRequest {
    fn from(value: reqwest::Request) -> Self {
        Self {
            request: value,
            options: RequestOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RequestOptions {
    pub use_semaphore: bool,
    pub max_retries: usize,
    pub own_connection: bool,
}

impl Default for RequestOptions {
    fn default() -> Self {
        Self {
            use_semaphore: true,
            max_retries: 1,
            own_connection: false,
        }
    }
}

impl RequestOptions {
    pub fn use_semaphore(mut self, should_use: bool) -> Self {
        self.use_semaphore = should_use;
        self
    }

    pub fn with_retries(mut self, retries: usize) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn own_connection(mut self, own: bool) -> Self {
        self.own_connection = own;
        self
    }
}
