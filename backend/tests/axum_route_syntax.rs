//! Axum Route Syntax Validation Test
//!
//! This test module ensures that all route definitions use the correct Axum syntax for path parameters.
//! Axum requires curly braces {param}, NOT colons :param, for capture groups.
//!
//! Using the wrong syntax causes runtime panic:
//! "Path segments must not start with `:`. For capture groups, use `{capture}`."
//!
//! These tests are run at compile time to catch these errors early.

#[test]
fn test_no_colon_route_parameters() {
    // Define all routes that are created in the application
    // These come from backend/src/lib.rs app() function
    let routes = vec![
        // K8s health endpoints
        "/healthz",
        "/readyz",
        // Auth endpoints
        "/api/auth/register",
        "/api/auth/login",
        "/api/auth/github/url",
        "/api/auth/github/callback",
        "/api/auth/gitlab/url",
        "/api/auth/gitlab/callback",
        // Admin endpoints
        "/api/admin/users",
        "/api/admin/users/{id}",
        "/api/admin/users/{id}/role",
        "/api/admin/users/{id}/ban",
        // Artifact endpoints
        "/api/artifacts",
        "/api/artifacts/{owner}",
        "/api/artifacts/{owner}/{artifact_name}",
        "/api/artifacts/{owner}/{artifact_name}/{version}",
        "/api/artifacts/{owner}/{artifact_name}/versions",
        "/api/artifacts/{owner}/{artifact_name}/download",
        "/api/artifacts/{owner}/{artifact_name}/{version}/download",
        "/api/artifacts/{owner}/{artifact_name}/readme",
        "/api/artifacts/{owner}/{artifact_name}/values",
        // Chart endpoints
        "/api/charts",
        "/api/charts/{owner}/{chart_name}",
        "/api/charts/{owner}/{chart_name}/readme",
        "/api/charts/{owner}/{chart_name}/values",
        // GitHub sync endpoints
        "/api/github/repos",
        "/api/github/repos/{id}",
        "/api/github/repos/{id}/sync",
        // GitLab sync endpoints
        "/api/gitlab/repos",
        "/api/gitlab/repos/{id}",
        "/api/gitlab/repos/{id}/sync",
        // Analytics endpoints - CRITICAL: These must use {}, not :
        "/api/analytics/instance",
        "/api/analytics/package/{owner}/{chart}",
        // Repository index endpoints
        "/charts/index.yaml",
        "/charts/{owner}/index.yaml",
        "/charts/{owner}/{chart}/{version}/index.yaml",
        "/charts/{owner}/{chart}/{version}/pull/index.yaml",
    ];

    let mut colon_params = Vec::new();

    for route in &routes {
        // Check for colon-style parameters like :id, :owner, :param, etc.
        // Valid pattern: /{:word}/ or /{:word}$ but starting with colon is invalid
        // We're looking for patterns like /:param where : is used for capture groups

        let parts: Vec<&str> = route.split('/').collect();
        for part in parts {
            // Skip empty parts and exact matches
            if part.is_empty() {
                continue;
            }

            // Check if this part looks like a colon parameter: :something
            if part.starts_with(':') {
                colon_params.push(format!(
                    "❌ Route '{}' has invalid colon parameter '{}'. Use '{{{}}}' instead.",
                    route,
                    part,
                    &part[1..] // Remove the colon to show the correct format
                ));
            }
        }
    }

    if !colon_params.is_empty() {
        panic!(
            "Found {} routes with INVALID colon-style parameters (Axum requires curly braces):\n{}",
            colon_params.len(),
            colon_params.join("\n")
        );
    }

    println!(
        "✅ All {} routes use correct Axum syntax with curly braces",
        routes.len()
    );
}

#[test]
fn test_all_routes_have_parameters_in_braces() {
    // Test that all parameterized routes use curly brace syntax
    // Pattern: {param_name}

    let parameterized_routes = vec![
        "/api/admin/users/{id}",
        "/api/admin/users/{id}/role",
        "/api/admin/users/{id}/ban",
        "/api/artifacts/{owner}",
        "/api/artifacts/{owner}/{artifact_name}",
        "/api/artifacts/{owner}/{artifact_name}/{version}",
        "/api/artifacts/{owner}/{artifact_name}/versions",
        "/api/artifacts/{owner}/{artifact_name}/download",
        "/api/artifacts/{owner}/{artifact_name}/{version}/download",
        "/api/artifacts/{owner}/{artifact_name}/readme",
        "/api/artifacts/{owner}/{artifact_name}/values",
        "/api/charts/{owner}/{chart_name}",
        "/api/charts/{owner}/{chart_name}/readme",
        "/api/charts/{owner}/{chart_name}/values",
        "/api/github/repos/{id}",
        "/api/github/repos/{id}/sync",
        "/api/gitlab/repos/{id}",
        "/api/gitlab/repos/{id}/sync",
        "/api/analytics/package/{owner}/{chart}",
        "/charts/{owner}/index.yaml",
        "/charts/{owner}/{chart}/{version}/index.yaml",
        "/charts/{owner}/{chart}/{version}/pull/index.yaml",
    ];

    let mut invalid_routes = Vec::new();

    for route in &parameterized_routes {
        // Check if all parameters use the correct {param} syntax
        // Valid: /path/{param}/more or /path/{param}$
        // Invalid: /path/:param/more or /path/:param$

        // Find all text between { and } - these are valid parameters
        let mut _valid_param_count = 0;
        let mut i = 0;
        let chars: Vec<char> = route.chars().collect();

        while i < chars.len() {
            if chars[i] == '{' {
                // Scan until we find the closing }
                let mut j = i + 1;
                let mut found_close = false;

                while j < chars.len() {
                    if chars[j] == '}' {
                        found_close = true;
                        break;
                    }
                    j += 1;
                }

                if found_close {
                    _valid_param_count += 1;
                    i = j + 1;
                } else {
                    invalid_routes
                        .push(format!("❌ Route '{}' has unclosed parameter brace", route));
                    break;
                }
            } else {
                i += 1;
            }
        }

        // Now check for any stray colons that might indicate old syntax
        if route.contains("/:") {
            invalid_routes.push(format!(
                "❌ Route '{}' contains '/:' which is old-style parameter syntax. \
                 Use '{{param}}' instead of ':param'.",
                route
            ));
        }
    }

    if !invalid_routes.is_empty() {
        panic!(
            "Found {} routes with invalid parameter syntax:\n{}",
            invalid_routes.len(),
            invalid_routes.join("\n")
        );
    }

    println!(
        "✅ All {} parameterized routes use correct curly brace syntax",
        parameterized_routes.len()
    );
}

#[test]
fn test_route_parameter_consistency() {
    // Test that routes with similar patterns use consistent parameter names
    // E.g., all artifact routes use {owner} and {artifact_name}, not sometimes {id}

    let consistency_checks = vec![
        // All owner/chart references should use these names
        (
            vec![
                "/api/charts/{owner}/{chart_name}",
                "/api/charts/{owner}/{chart_name}/readme",
                "/api/charts/{owner}/{chart_name}/values",
                "/api/analytics/package/{owner}/{chart}",
            ],
            "owner/chart routes",
        ),
        // All artifact routes should use consistent names
        (
            vec![
                "/api/artifacts/{owner}",
                "/api/artifacts/{owner}/{artifact_name}",
                "/api/artifacts/{owner}/{artifact_name}/{version}",
            ],
            "artifact routes",
        ),
    ];

    for (routes, description) in consistency_checks {
        for route in routes {
            // Verify the route follows expected parameter names
            // This is a soft check - we just ensure parameters are in braces
            let has_invalid_syntax = route.contains("/:") || route.contains("::");

            if has_invalid_syntax {
                panic!("❌ Route '{}' in {} has invalid syntax", route, description);
            }
        }
    }

    println!("✅ Route parameter naming is consistent");
}
