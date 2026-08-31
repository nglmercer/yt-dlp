/// Select and invoke the most-preferred handler for a request.
pub struct RequestDirector {
    handlers: Vec<Box<dyn RequestHandler>>,
}

impl RequestDirector {
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Construct the native HTTP stack with direct HTTP first and the
    /// HTTPS/proxy/compression-capable native backend as the secondary handler.
    pub fn native() -> Self {
        let mut director = Self::new();
        director.add_handler(HttpHandler);
        director.add_handler(ReqwestHandler);
        director
    }

    pub fn add_handler<H>(&mut self, handler: H)
    where
        H: RequestHandler + 'static,
    {
        self.handlers.push(Box::new(handler));
    }

    pub fn handler_count(&self) -> usize {
        self.handlers.len()
    }

    pub fn send(&self, request: &Request) -> Result<Response, RequestError> {
        if self.handlers.is_empty() {
            return Err(RequestError::new(
                ErrorKind::NoSupportingHandlers,
                "No request handlers configured",
            ));
        }

        let mut handlers: Vec<&dyn RequestHandler> =
            self.handlers.iter().map(AsRef::as_ref).collect();
        handlers.sort_by_key(|handler| std::cmp::Reverse(handler.preference(request)));

        let mut unsupported = Vec::new();
        for handler in handlers {
            match handler.supports(request) {
                Ok(()) => {}
                Err(error) if error.kind == ErrorKind::Unsupported => {
                    unsupported.push(format!("{}: {}", handler.name(), error.message));
                    continue;
                }
                Err(error) => return Err(error),
            }

            return handler.send(request);
        }

        let message = if unsupported.is_empty() {
            "Unable to handle request".to_owned()
        } else {
            format!("Unable to handle request: {}", unsupported.join(", "))
        };
        Err(RequestError::new(ErrorKind::NoSupportingHandlers, message))
    }
}

impl Default for RequestDirector {
    fn default() -> Self {
        Self::new()
    }
}
