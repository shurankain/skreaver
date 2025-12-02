//! OpenAPI documentation UI components

use axum::{
    Json,
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// UI feature flag for interactive testing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TryItOutMode {
    /// Interactive testing enabled
    Enabled,
    /// Interactive testing disabled (read-only)
    Disabled,
}

impl TryItOutMode {
    /// Check if try-it-out is enabled
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

impl Default for TryItOutMode {
    fn default() -> Self {
        Self::Enabled
    }
}

/// Validation mode for API requests/responses
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    /// Client-side validation enabled
    Enabled,
    /// Client-side validation disabled
    Disabled,
}

impl ValidationMode {
    /// Check if validation is enabled
    pub fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

impl Default for ValidationMode {
    fn default() -> Self {
        Self::Enabled
    }
}

/// Header visibility configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeaderVisibility {
    /// Show both request and response headers
    Both,
    /// Show only request headers
    RequestOnly,
    /// Show only response headers
    ResponseOnly,
    /// Hide all headers
    None,
}

impl HeaderVisibility {
    /// Check if request headers should be shown
    pub fn show_request(self) -> bool {
        matches!(self, Self::Both | Self::RequestOnly)
    }

    /// Check if response headers should be shown
    pub fn show_response(self) -> bool {
        matches!(self, Self::Both | Self::ResponseOnly)
    }
}

impl Default for HeaderVisibility {
    fn default() -> Self {
        Self::Both
    }
}

/// API documentation UI configuration
///
/// Uses type-safe enums instead of booleans to make feature flags more explicit
/// and prevent invalid combinations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiUiConfig {
    /// UI title
    pub title: String,
    /// API specification URL
    pub spec_url: String,
    /// Interactive testing mode
    pub try_it_out: TryItOutMode,
    /// Request/response validation mode
    pub validation: ValidationMode,
    /// Default expanded depth
    pub default_expanded_depth: u32,
    /// Header visibility configuration
    pub headers: HeaderVisibility,
    /// Custom CSS URL
    pub custom_css: Option<String>,
    /// Custom JS URL
    pub custom_js: Option<String>,
}

impl Default for ApiUiConfig {
    fn default() -> Self {
        Self {
            title: "Skreaver API Documentation".to_string(),
            spec_url: "/openapi.json".to_string(),
            try_it_out: TryItOutMode::default(),
            validation: ValidationMode::default(),
            default_expanded_depth: 1,
            headers: HeaderVisibility::default(),
            custom_css: None,
            custom_js: None,
        }
    }
}

impl ApiUiConfig {
    /// Create a read-only configuration (no interactive testing)
    pub fn read_only() -> Self {
        Self {
            try_it_out: TryItOutMode::Disabled,
            validation: ValidationMode::Disabled,
            headers: HeaderVisibility::None,
            ..Default::default()
        }
    }

    /// Create a minimal configuration (no headers, no validation)
    pub fn minimal() -> Self {
        Self {
            validation: ValidationMode::Disabled,
            headers: HeaderVisibility::None,
            ..Default::default()
        }
    }

    /// Create a development-friendly configuration (all features enabled)
    pub fn development() -> Self {
        Self {
            try_it_out: TryItOutMode::Enabled,
            validation: ValidationMode::Enabled,
            headers: HeaderVisibility::Both,
            default_expanded_depth: 2,
            ..Default::default()
        }
    }
}

/// Swagger UI implementation
pub struct SwaggerUi {
    config: ApiUiConfig,
    custom_config: HashMap<String, Value>,
}

impl SwaggerUi {
    /// Create a new Swagger UI instance
    pub fn new(config: ApiUiConfig) -> Self {
        Self {
            config,
            custom_config: HashMap::new(),
        }
    }

    /// Add custom configuration
    pub fn with_config(mut self, key: String, value: Value) -> Self {
        self.custom_config.insert(key, value);
        self
    }

    /// Generate the Swagger UI HTML
    pub fn generate_html(&self) -> Html<String> {
        let config_json = self.generate_config_json();

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <link rel="stylesheet" type="text/css" href="https://unpkg.com/swagger-ui-dist@5.10.5/swagger-ui.css" />
    <link rel="icon" type="image/png" href="https://unpkg.com/swagger-ui-dist@5.10.5/favicon-32x32.png" sizes="32x32" />
    <link rel="icon" type="image/png" href="https://unpkg.com/swagger-ui-dist@5.10.5/favicon-16x16.png" sizes="16x16" />
    {custom_css}
    <style>
        html {{
            box-sizing: border-box;
            overflow: -moz-scrollbars-vertical;
            overflow-y: scroll;
        }}
        *, *:before, *:after {{
            box-sizing: inherit;
        }}
        body {{
            margin:0;
            background: #fafafa;
        }}
        .swagger-ui .topbar {{
            background-color: #1976d2;
        }}
        .swagger-ui .topbar .download-url-wrapper .select-label {{
            color: #fff;
        }}
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5.10.5/swagger-ui-bundle.js"></script>
    <script src="https://unpkg.com/swagger-ui-dist@5.10.5/swagger-ui-standalone-preset.js"></script>
    <script>
        window.onload = function() {{
            const ui = SwaggerUIBundle({{
                url: '{spec_url}',
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [
                    SwaggerUIBundle.plugins.DownloadUrl
                ],
                layout: "StandaloneLayout",
                tryItOutEnabled: {try_it_out},
                validatorUrl: {validator_url},
                defaultModelsExpandDepth: {expanded_depth},
                showRequestHeaders: {show_request_headers},
                showResponseHeaders: {show_response_headers},
                docExpansion: "list",
                filter: true,
                showExtensions: true,
                showCommonExtensions: true,
                {custom_config}
            }});
            
            // Add custom request interceptor for authentication
            ui.getConfigs().requestInterceptor = function(request) {{
                // Add any custom headers or authentication here
                return request;
            }};
        }};
    </script>
    {custom_js}
</body>
</html>"#,
            title = self.config.title,
            spec_url = self.config.spec_url,
            try_it_out = self.config.try_it_out.is_enabled(),
            validator_url = if self.config.validation.is_enabled() {
                "null"
            } else {
                "false"
            },
            expanded_depth = self.config.default_expanded_depth,
            show_request_headers = self.config.headers.show_request(),
            show_response_headers = self.config.headers.show_response(),
            custom_css = self
                .config
                .custom_css
                .as_ref()
                .map(|url| format!(
                    r#"<link rel="stylesheet" type="text/css" href="{}" />"#,
                    url
                ))
                .unwrap_or_default(),
            custom_js = self
                .config
                .custom_js
                .as_ref()
                .map(|url| format!(r#"<script src="{}"></script>"#, url))
                .unwrap_or_default(),
            custom_config = config_json
        );

        Html(html)
    }

    /// Generate custom configuration JSON
    fn generate_config_json(&self) -> String {
        if self.custom_config.is_empty() {
            return String::new();
        }

        let config_items: Vec<String> = self
            .custom_config
            .iter()
            .map(|(key, value)| format!("{}: {}", key, value))
            .collect();

        if config_items.is_empty() {
            String::new()
        } else {
            format!("{},", config_items.join(",\n                "))
        }
    }
}

/// RapiDoc UI implementation (alternative to Swagger UI)
pub struct RapiDocUi {
    config: ApiUiConfig,
    theme: RapiDocTheme,
}

/// RapiDoc theme options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RapiDocTheme {
    Light,
    Dark,
    Auto,
}

impl Default for RapiDocTheme {
    fn default() -> Self {
        Self::Auto
    }
}

impl RapiDocUi {
    /// Create a new RapiDoc UI instance
    pub fn new(config: ApiUiConfig) -> Self {
        Self {
            config,
            theme: RapiDocTheme::default(),
        }
    }

    /// Set the theme
    pub fn with_theme(mut self, theme: RapiDocTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Generate the RapiDoc UI HTML
    pub fn generate_html(&self) -> Html<String> {
        let theme_str = match self.theme {
            RapiDocTheme::Light => "light",
            RapiDocTheme::Dark => "dark",
            RapiDocTheme::Auto => "auto",
        };

        let nav_bg_color = "#1976d2";
        let nav_text_color = "#ffffff";
        let nav_hover_bg_color = "#1565c0";
        let nav_hover_text_color = "#ffffff";
        let nav_accent_color = "#ffab40";
        let primary_color = "#1976d2";
        let text_color = "#333333";
        let bg_color = "#ffffff";
        let header_color = "#1976d2";
        let regular_font = "Open Sans";
        let nav_logo_slot = "nav-logo";

        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <script type="module" src="https://unpkg.com/rapidoc@9.3.4/dist/rapidoc-min.js"></script>
    {custom_css}
    <style>
        rapi-doc {{
            height: 100vh;
            width: 100%;
        }}
    </style>
</head>
<body>
    <rapi-doc
        spec-url="{spec_url}"
        theme="{theme}"
        render-style="read"
        show-info="true"
        show-components="true"
        show-header="true"
        allow-try="true"
        allow-server-selection="true"
        allow-authentication="true"
        allow-spec-file-download="true"
        show-curl-before-try="true"
        schema-style="tree"
        schema-expand-level="{expanded_level}"
        schema-description-expanded="true"
        api-key-name="X-API-Key"
        api-key-location="header"
        api-key-value=""
        default-schema-tab="schema"
        response-area-height="400px"
        nav-bg-color="{nav_bg_color}"
        nav-text-color="{nav_text_color}"
        nav-hover-bg-color="{nav_hover_bg_color}"
        nav-hover-text-color="{nav_hover_text_color}"
        nav-accent-color="{nav_accent_color}"
        primary-color="{primary_color}"
        text-color="{text_color}"
        bg-color="{bg_color}"
        header-color="{header_color}"
        regular-font="{regular_font}"
        mono-font="Monaco"
        font-size="default"
        sort-tags="false"
        sort-endpoints-by="method"
        goto-path=""
        fill-request-fields-with-example="true"
        persist-auth="false"
        {custom_attributes}
    >
        <div slot="{nav_logo_slot}" style="display: flex; align-items: center; justify-content: center; padding: 16px;">
            <div style="font-size: 18px; font-weight: bold; color: white;">Skreaver API</div>
        </div>
    </rapi-doc>
    {custom_js}
</body>
</html>"#,
            title = self.config.title,
            spec_url = self.config.spec_url,
            theme = theme_str,
            expanded_level = self.config.default_expanded_depth,
            nav_bg_color = nav_bg_color,
            nav_text_color = nav_text_color,
            nav_hover_bg_color = nav_hover_bg_color,
            nav_hover_text_color = nav_hover_text_color,
            nav_accent_color = nav_accent_color,
            primary_color = primary_color,
            text_color = text_color,
            bg_color = bg_color,
            header_color = header_color,
            regular_font = regular_font,
            nav_logo_slot = nav_logo_slot,
            custom_css = self
                .config
                .custom_css
                .as_ref()
                .map(|url| format!(
                    r#"<link rel="stylesheet" type="text/css" href="{}" />"#,
                    url
                ))
                .unwrap_or_default(),
            custom_js = self
                .config
                .custom_js
                .as_ref()
                .map(|url| format!(r#"<script src="{}"></script>"#, url))
                .unwrap_or_default(),
            custom_attributes = "", // For future extensibility
        );

        Html(html)
    }
}

/// API specification response helper
pub struct ApiSpecResponse;

impl ApiSpecResponse {
    /// Create a JSON response for the OpenAPI specification
    pub fn json(spec: Value) -> Response {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            "application/json; charset=utf-8".parse().unwrap(),
        );
        headers.insert(
            header::CACHE_CONTROL,
            "public, max-age=3600".parse().unwrap(),
        );

        (StatusCode::OK, headers, Json(spec)).into_response()
    }

    /// Create a YAML response for the OpenAPI specification
    pub fn yaml(spec: Value) -> Result<Response, Box<dyn std::error::Error>> {
        let yaml_str = serde_yaml::to_string(&spec)?;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_TYPE,
            "application/x-yaml; charset=utf-8".parse().unwrap(),
        );
        headers.insert(
            header::CACHE_CONTROL,
            "public, max-age=3600".parse().unwrap(),
        );

        Ok((StatusCode::OK, headers, yaml_str).into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_ui_config_default() {
        let config = ApiUiConfig::default();
        assert_eq!(config.title, "Skreaver API Documentation");
        assert_eq!(config.spec_url, "/openapi.json");
        assert_eq!(config.try_it_out, TryItOutMode::Enabled);
        assert_eq!(config.validation, ValidationMode::Enabled);
        assert_eq!(config.headers, HeaderVisibility::Both);
    }

    #[test]
    fn test_try_it_out_mode() {
        assert!(TryItOutMode::Enabled.is_enabled());
        assert!(!TryItOutMode::Disabled.is_enabled());
    }

    #[test]
    fn test_validation_mode() {
        assert!(ValidationMode::Enabled.is_enabled());
        assert!(!ValidationMode::Disabled.is_enabled());
    }

    #[test]
    fn test_header_visibility() {
        assert!(HeaderVisibility::Both.show_request());
        assert!(HeaderVisibility::Both.show_response());

        assert!(HeaderVisibility::RequestOnly.show_request());
        assert!(!HeaderVisibility::RequestOnly.show_response());

        assert!(!HeaderVisibility::ResponseOnly.show_request());
        assert!(HeaderVisibility::ResponseOnly.show_response());

        assert!(!HeaderVisibility::None.show_request());
        assert!(!HeaderVisibility::None.show_response());
    }

    #[test]
    fn test_api_ui_config_presets() {
        let read_only = ApiUiConfig::read_only();
        assert_eq!(read_only.try_it_out, TryItOutMode::Disabled);
        assert_eq!(read_only.validation, ValidationMode::Disabled);
        assert_eq!(read_only.headers, HeaderVisibility::None);

        let minimal = ApiUiConfig::minimal();
        assert_eq!(minimal.try_it_out, TryItOutMode::Enabled);
        assert_eq!(minimal.validation, ValidationMode::Disabled);
        assert_eq!(minimal.headers, HeaderVisibility::None);

        let dev = ApiUiConfig::development();
        assert_eq!(dev.try_it_out, TryItOutMode::Enabled);
        assert_eq!(dev.validation, ValidationMode::Enabled);
        assert_eq!(dev.headers, HeaderVisibility::Both);
        assert_eq!(dev.default_expanded_depth, 2);
    }

    #[test]
    fn test_swagger_ui_creation() {
        let config = ApiUiConfig::default();
        let ui = SwaggerUi::new(config);
        assert!(ui.custom_config.is_empty());
    }

    #[test]
    fn test_swagger_ui_with_custom_config() {
        let config = ApiUiConfig::default();
        let ui =
            SwaggerUi::new(config).with_config("customParam".to_string(), serde_json::json!(true));
        assert!(ui.custom_config.contains_key("customParam"));
    }

    #[test]
    fn test_rapidoc_ui_creation() {
        let config = ApiUiConfig::default();
        let ui = RapiDocUi::new(config);
        assert!(matches!(ui.theme, RapiDocTheme::Auto));
    }

    #[test]
    fn test_rapidoc_ui_with_theme() {
        let config = ApiUiConfig::default();
        let ui = RapiDocUi::new(config).with_theme(RapiDocTheme::Dark);
        assert!(matches!(ui.theme, RapiDocTheme::Dark));
    }

    #[test]
    fn test_swagger_ui_html_generation() {
        let config = ApiUiConfig::default();
        let ui = SwaggerUi::new(config);
        let html = ui.generate_html();
        let content = html.0;

        assert!(content.contains("swagger-ui"));
        assert!(content.contains("/openapi.json"));
        assert!(content.contains("Skreaver API Documentation"));
    }

    #[test]
    fn test_rapidoc_ui_html_generation() {
        let config = ApiUiConfig::default();
        let ui = RapiDocUi::new(config);
        let html = ui.generate_html();
        let content = html.0;

        assert!(content.contains("rapi-doc"));
        assert!(content.contains("/openapi.json"));
        assert!(content.contains("Skreaver API Documentation"));
    }
}
