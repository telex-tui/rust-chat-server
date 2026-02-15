/// A message filter — a closure that can inspect and optionally modify messages.
///
/// Filters use FnMut because they may maintain state (e.g., counting
/// messages, tracking timestamps). FnMut allows both reading and writing
/// captured variables. Fn would be too restrictive (no mutation),
/// FnOnce would be consumed after a single call.
///
/// Stored as Box<dyn FnMut> because closures have anonymous types —
/// you can't name them. Boxing erases the type and lets us store
/// different closures in a Vec.
pub struct FilterRegistry {
    filters: Vec<Box<dyn FnMut(&str, &str) -> FilterAction>>,
}

/// What a filter decides to do with a message.
pub enum FilterAction {
    /// Let the message through unchanged.
    Allow,
    /// Replace the message body with this text.
    Modify(String),
    /// Block the message entirely.
    Block(String),
}

impl FilterRegistry {
    pub fn new() -> Self {
        Self {
            filters: Vec::new(),
        }
    }

    /// Register a filter. Takes any closure that matches the signature.
    /// The closure receives (username, body) and returns a FilterAction.
    pub fn add<F>(&mut self, filter: F)
    where
        F: FnMut(&str, &str) -> FilterAction + 'static,
    {
        self.filters.push(Box::new(filter));
    }

    /// Run all filters on a message. Returns the final action.
    pub fn apply(&mut self, username: &str, body: &str) -> FilterAction {
        let mut current_body = body.to_string();

        for filter in &mut self.filters {
            match filter(username, &current_body) {
                FilterAction::Allow => {}
                FilterAction::Modify(new_body) => {
                    current_body = new_body;
                }
                FilterAction::Block(reason) => {
                    return FilterAction::Block(reason);
                }
            }
        }

        if current_body != body {
            FilterAction::Modify(current_body)
        } else {
            FilterAction::Allow
        }
    }
}
