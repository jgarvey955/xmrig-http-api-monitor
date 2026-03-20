#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiOptionSpec {
    pub key: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiEndpointSpec {
    pub method: String,
    pub path: String,
    pub description: String,
    pub safe_to_poll: bool,
}

pub fn api_documentation() -> &'static str {
    include_str!("../http-api.output")
}

#[cfg(test)]
pub fn load_api_options() -> Vec<ApiOptionSpec> {
    parse_bulleted_section("Current JSON config keys")
        .into_iter()
        .map(|(key, description)| ApiOptionSpec { key, description })
        .collect()
}

pub fn load_api_endpoints() -> Vec<ApiEndpointSpec> {
    parse_bulleted_section("Implemented HTTP API routes")
        .into_iter()
        .filter_map(|(entry, description)| {
            let (method, path) = entry.split_once(' ')?;

            Some(ApiEndpointSpec {
                method: method.trim().to_string(),
                path: path.trim().to_string(),
                description,
                safe_to_poll: method.trim() == "GET",
            })
        })
        .collect()
}

#[cfg(test)]
pub fn documented_write_routes() -> Vec<ApiEndpointSpec> {
    load_api_endpoints()
        .into_iter()
        .filter(|endpoint| !endpoint.safe_to_poll && endpoint.path.starts_with('/'))
        .collect()
}

pub fn default_endpoint(endpoints: &[ApiEndpointSpec]) -> Option<String> {
    endpoints
        .iter()
        .find(|endpoint| endpoint.method == "GET" && endpoint.path == "/1/summary")
        .map(|endpoint| endpoint.path.clone())
        .or_else(|| {
            endpoints
                .iter()
                .find(|endpoint| endpoint.safe_to_poll)
                .map(|endpoint| endpoint.path.clone())
        })
}

fn parse_bulleted_section(title: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_description = Vec::new();
    let mut in_section = false;

    for line in api_documentation().lines() {
        let trimmed = line.trim_end();

        if trimmed == title {
            in_section = true;
            continue;
        }

        if !in_section {
            continue;
        }

        if !trimmed.is_empty() && !line.starts_with(' ') && !line.starts_with('-') {
            break;
        }

        if let Some(value) = trimmed.trim().strip_prefix("- ") {
            if let Some(title) = current_title.take() {
                entries.push((title, current_description.join(" ")));
            }

            current_title = Some(value.trim().to_string());
            current_description.clear();
            continue;
        }

        if !trimmed.trim().is_empty() {
            current_description.push(trimmed.trim().to_string());
        }
    }

    if let Some(title) = current_title {
        entries.push((title, current_description.join(" ")));
    }

    entries
}

#[cfg(test)]
mod tests {
    use super::{default_endpoint, documented_write_routes, load_api_endpoints, load_api_options};

    #[test]
    fn parses_config_keys_from_http_api_output() {
        let options = load_api_options();

        assert!(options.iter().any(|option| option.key == "http.access-token"));
        assert!(options.iter().any(|option| option.key == "api.worker-id"));
    }

    #[test]
    fn parses_summary_route_and_default_endpoint() {
        let endpoints = load_api_endpoints();

        assert!(
            endpoints
                .iter()
                .any(|endpoint| endpoint.method == "GET" && endpoint.path == "/1/summary")
        );
        assert_eq!(default_endpoint(&endpoints).as_deref(), Some("/1/summary"));
    }

    #[test]
    fn extracts_documented_write_routes() {
        let write_routes = documented_write_routes();

        assert!(
            write_routes
                .iter()
                .any(|route| route.method == "POST" && route.path == "/1/config")
        );
        assert!(
            write_routes
                .iter()
                .any(|route| route.method == "POST" && route.path == "/json_rpc")
        );
    }
}
