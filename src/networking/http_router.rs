//! # HTTP Router Implementation
//!
//! Implements a Trie-based HTTP router with support for dynamic path parameters.
//!
//! **Replaces Crates:** `matchit`, `route-recognizer`
//!
//! **Real-world Usage:**
//! - Core component of web frameworks like Axum, Actix-web, and Gin (Go).
//!
//! **Why build it yourself?**
//! Routing is more than just regex matching. A Trie (Prefix Tree) allows for O(k) matching
//! where k is the number of path segments, independent of the number of registered routes.
//! You'll learn how to handle dynamic segments (`:id`), wildcards, and method dispatching efficiently.

use crate::networking::http_server::{Handler, HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::sync::Arc;

// =========================================================================================
// Architecture
// =========================================================================================
//
// Data Structure: Radix Trie (simplified)
// Each node in the trie represents a path segment.
//
// Root
//  ├── "users"
//  │    ├── :id (Dynamic) -> GET Handler, POST Handler
//  │    │    └── "settings" -> GET Handler
//  │    └── "new" -> POST Handler
//  └── "static" -> GET Handler
//
// Invariants:
// 1. A path is split by '/'.
// 2. Dynamic segments (starting with ':') capture the value in that position.
// 3. Only one dynamic segment is allowed per level (simplification).
//
// Complexity:
// ┌───────────────┬─────────────┬─────────────┐
// │ Operation     │ Time        │ Space       │
// ├───────────────┼─────────────┼─────────────┤
// │ Add Route     │ O(k)        │ O(k)        │
// │ Match Route   │ O(k)        │ O(1)        │
// └───────────────┴─────────────┴─────────────┘
// where k is the number of path segments.

/// A node in the routing Trie.
#[derive(Default)]
struct Node {
    /// Children nodes keyed by static path segment.
    children: HashMap<String, Node>,
    /// Optional child node for dynamic parameter (e.g., ":id").
    /// We store the parameter name (without ':') and the node.
    /// Limitation: Only one dynamic parameter per level.
    dynamic_child: Option<(String, Box<Node>)>,
    /// Handlers for this path, keyed by HTTP method (GET, POST, etc.).
    handlers: HashMap<String, Arc<dyn Handler>>,
}

/// A Trie-based HTTP Router.
pub struct Router {
    root: Node,
}

impl Router {
    /// Creates a new empty Router.
    pub fn new() -> Self {
        Self {
            root: Node::default(),
        }
    }

    /// Registers a handler for a specific method and path.
    ///
    /// Path examples:
    /// - `/`
    /// - `/users`
    /// - `/users/:id`
    /// - `/users/:id/profile`
    pub fn add_route<H: Handler>(&mut self, method: &str, path: &str, handler: H) {
        // ⚡ BOLT OPTIMIZATION: Avoid intermediate `.collect::<Vec<&str>>()` allocation.
        // By iterating over the `Split` iterator directly, we eliminate an O(N) heap allocation per route registration.
        let parts = path.split('/').filter(|p| !p.is_empty());
        let mut current = &mut self.root;

        for part in parts {
            if part.starts_with(':') {
                // Dynamic segment
                let param_name = part[1..].to_string();

                // GOTCHA: If we already have a dynamic child, it must match the new one's name.
                // In a production router, we might allow different names if they don't conflict,
                // or return a Result. Here we panic on conflict for simplicity.
                if let Some((existing_name, _)) = &current.dynamic_child
                    && existing_name != &param_name
                {
                    panic!(
                        "Conflict: Route already has dynamic parameter '{}', cannot add '{}'",
                        existing_name, param_name
                    );
                }

                if current.dynamic_child.is_none() {
                    current.dynamic_child = Some((param_name, Box::new(Node::default())));
                }

                // Move to the dynamic child
                // Unwrap is safe because we just ensured it exists.
                // We need to match again to get the mutable reference from the Box.
                if let Some((_, ref mut child_node)) = current.dynamic_child {
                    current = child_node;
                } else {
                    unreachable!();
                }
            } else {
                // Static segment
                current = current.children.entry(part.to_string()).or_default();
            }
        }

        // We are at the leaf node for this path. Register the handler.
        // Normalize method to uppercase.
        current
            .handlers
            .insert(method.to_uppercase(), Arc::new(handler));
    }

    /// Matches a request method and path to a registered handler.
    /// Returns the handler and extracted path parameters.
    pub fn match_route(
        &self,
        method: &str,
        path: &str,
    ) -> Option<(Arc<dyn Handler>, HashMap<String, String>)> {
        // ⚡ BOLT OPTIMIZATION: Avoid intermediate `.collect::<Vec<&str>>()` allocation.
        // By iterating over the `Split` iterator directly, we eliminate an O(N) heap allocation per route match.
        let parts = path.split('/').filter(|p| !p.is_empty());
        let mut current = &self.root;
        let mut params = HashMap::new();

        for part in parts {
            if let Some(child) = current.children.get(part) {
                current = child;
            } else if let Some((param_name, child_node)) = &current.dynamic_child {
                params.insert(param_name.clone(), part.to_string());
                current = child_node;
            } else {
                return None;
            }
        }

        current
            .handlers
            .get(&method.to_uppercase())
            .map(|h| (Arc::clone(h), params))
    }
}

// Ensure Router can be sent across threads
unsafe impl Send for Router {}
unsafe impl Sync for Router {}

impl Handler for Router {
    fn handle(&self, mut req: HttpRequest) -> HttpResponse {
        // Strip query parameters for matching.
        // We use a block to limit the borrow scope of `req.path`.
        let route_match = {
            let path = req.path.split('?').next().unwrap_or(&req.path);
            self.match_route(&req.method, path)
        };

        if let Some((handler, params)) = route_match {
            req.params = params;
            handler.handle(req)
        } else {
            HttpResponse::new(404, "Not Found", Some(b"404 Not Found".to_vec()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler {
        name: String,
    }

    impl Handler for MockHandler {
        fn handle(&self, req: HttpRequest) -> HttpResponse {
            let body = format!("Handler: {}, Params: {:?}", self.name, req.params);
            HttpResponse::new(200, "OK", Some(body.into_bytes()))
        }
    }

    fn make_req(method: &str, path: &str) -> HttpRequest {
        HttpRequest {
            method: method.to_string(),
            path: path.to_string(),
            version: "HTTP/1.1".to_string(),
            headers: HashMap::new(),
            params: HashMap::new(),
            body: Vec::new(),
        }
    }

    #[test]
    fn test_static_routes() {
        let mut router = Router::new();
        router.add_route(
            "GET",
            "/home",
            MockHandler {
                name: "Home".into(),
            },
        );
        router.add_route(
            "POST",
            "/submit",
            MockHandler {
                name: "Submit".into(),
            },
        );

        let req = make_req("GET", "/home");
        let resp = router.handle(req);
        assert_eq!(resp.status_code, 200);
        assert!(
            String::from_utf8(resp.body.unwrap())
                .unwrap()
                .contains("Handler: Home")
        );

        let req = make_req("POST", "/submit");
        let resp = router.handle(req);
        assert_eq!(resp.status_code, 200);
        assert!(
            String::from_utf8(resp.body.unwrap())
                .unwrap()
                .contains("Handler: Submit")
        );
    }

    #[test]
    fn test_dynamic_routes() {
        let mut router = Router::new();
        router.add_route(
            "GET",
            "/users/:id",
            MockHandler {
                name: "User".into(),
            },
        );

        let req = make_req("GET", "/users/123");
        let resp = router.handle(req);
        assert_eq!(resp.status_code, 200);
        let body = String::from_utf8(resp.body.unwrap()).unwrap();
        assert!(body.contains("Handler: User"));
        assert!(body.contains("\"id\": \"123\""));
    }

    #[test]
    fn test_nested_dynamic_routes() {
        let mut router = Router::new();
        router.add_route(
            "GET",
            "/users/:user_id/posts/:post_id",
            MockHandler {
                name: "Post".into(),
            },
        );

        let req = make_req("GET", "/users/42/posts/99");
        let resp = router.handle(req);
        assert_eq!(resp.status_code, 200);
        let body = String::from_utf8(resp.body.unwrap()).unwrap();
        assert!(body.contains("\"user_id\": \"42\""));
        assert!(body.contains("\"post_id\": \"99\""));
    }

    #[test]
    fn test_route_not_found() {
        let mut router = Router::new();
        router.add_route(
            "GET",
            "/home",
            MockHandler {
                name: "Home".into(),
            },
        );

        let req = make_req("GET", "/about");
        let resp = router.handle(req);
        assert_eq!(resp.status_code, 404);
    }

    #[test]
    fn test_method_mismatch() {
        let mut router = Router::new();
        router.add_route(
            "POST",
            "/submit",
            MockHandler {
                name: "Submit".into(),
            },
        );

        let req = make_req("GET", "/submit");
        let resp = router.handle(req);
        // Should be 404 (or 405 Method Not Allowed if we implemented that logic, but simple router returns None -> 404)
        assert_eq!(resp.status_code, 404);
    }

    #[test]
    fn test_query_parameters_ignored() {
        let mut router = Router::new();
        router.add_route(
            "GET",
            "/search",
            MockHandler {
                name: "Search".into(),
            },
        );

        let req = make_req("GET", "/search?q=rust");
        let resp = router.handle(req);
        assert_eq!(resp.status_code, 200);
        assert!(
            String::from_utf8(resp.body.unwrap())
                .unwrap()
                .contains("Handler: Search")
        );
    }

    #[test]
    fn test_static_shadows_dynamic() {
        let mut router = Router::new();
        router.add_route("GET", "/users/me", MockHandler { name: "Me".into() });
        router.add_route(
            "GET",
            "/users/:id",
            MockHandler {
                name: "User".into(),
            },
        );

        let req = make_req("GET", "/users/me");
        let resp = router.handle(req);
        assert!(
            String::from_utf8(resp.body.unwrap())
                .unwrap()
                .contains("Handler: Me")
        );

        let req = make_req("GET", "/users/bob");
        let resp = router.handle(req);
        let body = String::from_utf8(resp.body.unwrap()).unwrap();
        assert!(body.contains("Handler: User"));
        assert!(body.contains("\"id\": \"bob\""));
    }
}
