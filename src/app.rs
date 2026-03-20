use crate::inventory::{ApiEndpointSpec, default_endpoint, load_api_endpoints};
use crate::rpc::ApiClient;
use crate::settings::{Settings, WindowState};
use chrono::Local;
use iced::widget::{button, column, container, pick_list, row, scrollable, text, text_input};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Shadow, Size, Subscription, Task,
    Theme, window,
};
use serde_json::{Value, json};
use std::time::Duration;

const ACCENT: Color = Color::from_rgb(0.96, 0.47, 0.12);
const ACCENT_DIM: Color = Color::from_rgb(0.82, 0.37, 0.10);
const BG_APP: Color = Color::from_rgb(0.07, 0.07, 0.08);
const BG_PANEL: Color = Color::from_rgb(0.11, 0.11, 0.12);
const BG_PANEL_ALT: Color = Color::from_rgb(0.15, 0.15, 0.17);
const BG_SIDEBAR: Color = Color::from_rgb(0.08, 0.08, 0.09);
const BORDER_SOFT: Color = Color::from_rgba(1.0, 1.0, 1.0, 0.08);
const TEXT_MAIN: Color = Color::from_rgb(0.93, 0.93, 0.93);
const TEXT_MUTED: Color = Color::from_rgb(0.63, 0.65, 0.68);
const SUCCESS: Color = Color::from_rgb(0.21, 0.82, 0.47);
const DANGER: Color = Color::from_rgb(0.93, 0.27, 0.33);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Home,
    Api,
    Preferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Setup,
    Dashboard,
}

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Refresh,
    StatusTick,
    WindowResized(Size),
    CopyToClipboard(String),
    SelectView(View),
    SelectEndpoint(String),
    UpdateApiHost(String),
    UpdateApiPort(String),
    UpdateApiTransport(String),
    UpdateApiAccessToken(String),
    UpdatePollFrequency(String),
    PollSelectedEndpoint,
    SaveAndConnect,
    ExitRequested,
    ExitWindowResolved(Option<window::Id>),
}

#[derive(Debug, Clone, Default)]
struct SummarySnapshot {
    api_id: Option<String>,
    worker_id: Option<String>,
    version: Option<String>,
    kind: Option<String>,
    mode: Option<String>,
    uptime: Option<String>,
    restricted: Option<String>,
    features: Option<String>,
    hashrate_total: Option<String>,
    workers: Option<String>,
    miners_now: Option<String>,
    miners_max: Option<String>,
    upstream_active: Option<String>,
    upstream_ratio: Option<String>,
    accepted: Option<String>,
    rejected: Option<String>,
    invalid: Option<String>,
    expired: Option<String>,
    avg_time: Option<String>,
    latency: Option<String>,
    donate_level: Option<String>,
    donated: Option<String>,
    memory_rss: Option<String>,
    load_average: Option<String>,
}

#[derive(Debug, Clone)]
struct PollOutcome {
    summary: SummarySnapshot,
    summary_json: Value,
    captured_output: Option<Value>,
    notice: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Clone)]
struct OutputField {
    label: String,
    value: String,
}

pub struct AppState {
    screen: Screen,
    view: View,
    api_endpoints: Vec<ApiEndpointSpec>,
    connection_status: String,
    last_poll: String,
    summary: SummarySnapshot,
    last_summary_json: Option<Value>,
    selected_endpoint: String,
    selected_output: Option<Value>,
    error: Option<String>,
    notice: Option<String>,
    api_host_input: String,
    api_port_input: String,
    api_transport_input: String,
    api_access_token_input: String,
    poll_frequency_input: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self::init()
    }
}

impl AppState {
    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![window::resize_events().map(|(_id, size)| {
            Message::WindowResized(size)
        })];

        if self.screen == Screen::Dashboard {
            let seconds = self
                .poll_frequency_input
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|value| *value > 0)
                .unwrap_or(10);

            subscriptions.push(
                iced::time::every(Duration::from_secs(seconds)).map(|_| Message::StatusTick),
            );
        }

        Subscription::batch(subscriptions)
    }

    pub fn init() -> Self {
        let (settings, settings_exist, load_error) = match Settings::load() {
            Ok((settings, exists)) => (settings, exists, None),
            Err(error) => (
                Settings::default(),
                false,
                Some(format!("Failed to load settings: {error}")),
            ),
        };

        let api_endpoints = load_api_endpoints();
        let selected_endpoint = settings.preferred_endpoint.clone();
        let empty_if_missing = |value: String| {
            if settings_exist {
                value
            } else {
                String::new()
            }
        };

        let mut state = Self {
            screen: if settings_exist {
                Screen::Dashboard
            } else {
                Screen::Setup
            },
            view: View::Home,
            api_endpoints,
            connection_status: "Disconnected".into(),
            last_poll: "Never".into(),
            summary: SummarySnapshot::default(),
            last_summary_json: None,
            selected_endpoint,
            selected_output: None,
            error: load_error,
            notice: if settings_exist {
                None
            } else {
                Some(
                    "Enter the xmrigcc-proxy HTTP API settings and verify GET /1/summary."
                        .into(),
                )
            },
            api_host_input: empty_if_missing(settings.api_host.clone()),
            api_port_input: empty_if_missing(settings.api_port.to_string()),
            api_transport_input: empty_if_missing(settings.api_transport.clone()),
            api_access_token_input: empty_if_missing(settings.api_access_token.clone()),
            poll_frequency_input: empty_if_missing(settings.poll_frequency_seconds.to_string()),
        };

        state.ensure_selected_endpoint();

        if settings_exist {
            match state.connect_with_current_inputs() {
                Ok(()) => {
                    state.notice = Some(
                        "Saved settings loaded and GET /1/summary completed successfully.".into(),
                    );
                }
                Err(error) => {
                    state.error = Some(error);
                    state.notice =
                        Some("Saved settings loaded, but GET /1/summary is not reachable.".into());
                }
            }
        }

        state
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh | Message::StatusTick => {
                self.refresh_status();
                Task::none()
            }
            Message::WindowResized(size) => {
                self.persist_window_size(size);
                Task::none()
            }
            Message::CopyToClipboard(value) => iced::clipboard::write::<Message>(value),
            Message::SelectView(view) => {
                self.view = view;
                Task::none()
            }
            Message::SelectEndpoint(path) => {
                self.selected_endpoint = path;
                self.selected_output = if self.selected_endpoint == "/1/summary" {
                    self.last_summary_json.clone()
                } else {
                    None
                };
                Task::none()
            }
            Message::UpdateApiHost(value) => {
                self.api_host_input = value;
                Task::none()
            }
            Message::UpdateApiPort(value) => {
                self.api_port_input = value;
                Task::none()
            }
            Message::UpdateApiTransport(value) => {
                self.api_transport_input = value;
                Task::none()
            }
            Message::UpdateApiAccessToken(value) => {
                self.api_access_token_input = value;
                Task::none()
            }
            Message::UpdatePollFrequency(value) => {
                self.poll_frequency_input = value;
                Task::none()
            }
            Message::PollSelectedEndpoint => {
                self.manual_poll_selected_endpoint();
                Task::none()
            }
            Message::SaveAndConnect => {
                match self.save_and_connect() {
                    Ok(()) => {
                        self.screen = Screen::Dashboard;
                        self.view = View::Home;
                    }
                    Err(error) => self.error = Some(error),
                }
                Task::none()
            }
            Message::ExitRequested => window::latest().map(Message::ExitWindowResolved),
            Message::ExitWindowResolved(Some(id)) => window::close(id),
            Message::ExitWindowResolved(None) => Task::none(),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        match self.screen {
            Screen::Setup => self.setup_view(),
            Screen::Dashboard => self.dashboard_view(),
        }
    }

    fn setup_view(&self) -> Element<'_, Message> {
        let content = column![
            text("XMRIG HTTP API MONITOR").size(34).color(TEXT_MAIN),
            text("Initial HTTP API setup").size(16).color(ACCENT),
            text(
                "The monitor verifies GET /1/summary with the configured Bearer token before opening the dashboard."
            )
            .size(15)
            .color(TEXT_MUTED),
            self.settings_editor(false),
        ]
        .spacing(18)
        .max_width(960);

        container(
            scrollable(content)
                .direction(default_vertical_scroll_direction())
                .style(content_scrollable_style)
                .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill)
        .padding(28)
        .into()
    }

    fn dashboard_view(&self) -> Element<'_, Message> {
        let content = match self.view {
            View::Home => self.home_view(),
            View::Api => self.api_view(),
            View::Preferences => self.preferences_view(),
        };

        let body = row![self.sidebar(), content].spacing(18).height(Fill);

        container(column![self.title_bar(), body].spacing(18).padding([10, 14]))
            .width(Fill)
            .height(Fill)
            .style(panel_style(BG_APP, Some(TEXT_MAIN), None))
            .into()
    }

    fn title_bar(&self) -> Element<'_, Message> {
        let status_badge = container(text(&self.connection_status).size(13).color(TEXT_MAIN))
            .padding([6, 12])
            .style(panel_style(
                if self.connection_status == "Connected" {
                    SUCCESS
                } else {
                    ACCENT_DIM
                },
                Some(TEXT_MAIN),
                None,
            ));

        let status_group = row![
            column![text("HTTP API").size(12).color(TEXT_MUTED), status_badge].spacing(6),
            column![
                text("Last Poll").size(12).color(TEXT_MUTED),
                text(&self.last_poll).size(15).color(TEXT_MAIN),
            ]
            .spacing(6),
        ]
        .spacing(16)
        .align_y(Alignment::Center);

        let actions = row![
            self.menu_button("Home", Message::SelectView(View::Home), self.view == View::Home),
            self.menu_button(
                "XMRIG API",
                Message::SelectView(View::Api),
                self.view == View::Api,
            ),
            self.menu_button(
                "Preferences",
                Message::SelectView(View::Preferences),
                self.view == View::Preferences,
            ),
            self.menu_button("Refresh", Message::Refresh, false),
            self.menu_button("Exit", Message::ExitRequested, false),
        ]
        .spacing(10)
        .align_y(Alignment::Center);

        let bar = row![
            column![
                text("XMRIG").size(14).color(ACCENT),
                text("Monitor").size(26).color(TEXT_MAIN),
            ]
            .spacing(2),
            container(status_group).width(Fill).center_x(Fill),
            actions,
        ]
        .align_y(Alignment::Center)
        .spacing(16);

        container(bar)
            .padding([16, 18])
            .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(18.0)))
            .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let summary = container(
            column![
                text("XMRIG").size(15).color(TEXT_MUTED),
                text("HTTP API Overview").size(24).color(TEXT_MAIN),
                self.metric_line("Connection", &self.connection_status),
                self.metric_line("Version", self.display_value(self.summary.version.as_deref())),
                self.metric_line("Mode", self.display_value(self.summary.mode.as_deref())),
                self.metric_line("Workers", self.display_value(self.summary.workers.as_deref())),
                self.metric_line(
                    "Hashrate",
                    self.display_value(self.summary.hashrate_total.as_deref()),
                ),
                self.metric_line(
                    "Upstream Ratio",
                    self.display_value(self.summary.upstream_ratio.as_deref()),
                ),
            ]
            .spacing(10),
        )
        .padding(18)
        .style(panel_style(BG_PANEL_ALT, Some(TEXT_MAIN), Some(20.0)));

        let nav = column![
            self.nav_button("Home", View::Home),
            self.nav_button("XMRIG API", View::Api),
            self.nav_button("Preferences", View::Preferences),
        ]
        .spacing(8);

        let footer = container(
            column![
                text("Base URL").size(13).color(TEXT_MUTED),
                text(self.settings_snapshot().api_url_display())
                    .size(14)
                    .color(TEXT_MAIN),
                text("Health Route").size(13).color(TEXT_MUTED),
                text("/1/summary").size(14).color(TEXT_MAIN),
                text("Selected Route").size(13).color(TEXT_MUTED),
                text(&self.selected_endpoint).size(14).color(TEXT_MAIN),
                text(
                    self.notice
                        .as_deref()
                        .unwrap_or("Ready for manual API polling.")
                )
                .size(13)
                .color(TEXT_MUTED),
            ]
            .spacing(8),
        )
        .padding(16)
        .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(16.0)));

        container(column![summary, nav, footer].spacing(16))
            .width(300)
            .height(Fill)
            .style(panel_style(BG_SIDEBAR, Some(TEXT_MAIN), Some(24.0)))
            .padding(12)
            .into()
    }

    fn home_view(&self) -> Element<'_, Message> {
        let settings = self.settings_snapshot();

        let metrics = row![
            self.info_card("Proxy Version", self.display_value(self.summary.version.as_deref())),
            self.info_card("Workers", self.display_value(self.summary.workers.as_deref())),
            self.info_card(
                "Hashrate Total",
                self.display_value(self.summary.hashrate_total.as_deref()),
            ),
            self.info_card("Accepted", self.display_value(self.summary.accepted.as_deref())),
        ]
        .spacing(14);

        let status = container(
            column![
                text("Home").size(28).color(TEXT_MAIN),
                text(
                    "Connection status is derived from GET /1/summary. If that request succeeds, the proxy is treated as connected."
                )
                .size(15)
                .color(TEXT_MUTED),
                self.value_box("Base URL", settings.api_url_display()),
                self.value_box("Health Route", settings.summary_url_display()),
                self.summary_grid(),
                self.message_panel(),
            ]
            .spacing(16),
        )
        .padding(22)
        .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(22.0)));

        scrollable(column![metrics, status].spacing(16))
            .direction(default_vertical_scroll_direction())
            .style(content_scrollable_style)
            .width(Fill)
            .height(Fill)
            .into()
    }

    fn api_view(&self) -> Element<'_, Message> {
        let endpoint_options = self.safe_endpoint_paths();
        let selected = endpoint_options
            .iter()
            .find(|path| path.as_str() == self.selected_endpoint)
            .cloned();
        let route_description = self
            .selected_endpoint_spec()
            .map(|endpoint| endpoint.description.clone())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Documented in http-api.output.".to_string());

        let route_picker = pick_list(endpoint_options, selected, Message::SelectEndpoint)
            .placeholder("Select API route")
            .padding([10, 14])
            .text_size(15)
            .style(pick_list_style);

        let header = container(
            column![
                row![
                    text("XMRIG API").size(28).color(TEXT_MAIN),
                    route_picker,
                    button(text("Poll").size(14))
                        .padding([10, 14])
                        .style(secondary_button_style)
                        .on_press(Message::PollSelectedEndpoint),
                ]
                .spacing(18)
                .align_y(Alignment::Center),
                text(
                    "Poll a documented GET route and review each returned field as a labeled value."
                )
                .size(15)
                .color(TEXT_MUTED),
            ]
            .spacing(16),
        )
        .padding(22)
        .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(22.0)));

        let output = self.selected_output_copy_text();
        let output_fields = self.selected_output_fields();
        let mut output_items = column![
            self.value_box("Selected Route", self.selected_endpoint.clone()),
            self.value_box("Route Notes", route_description),
        ]
        .spacing(12);

        if output_fields.is_empty() {
            output_items = output_items.push(
                container(
                    text("No API response has been captured yet. Select a route and press Poll.")
                        .size(14)
                        .color(TEXT_MUTED),
                )
                .padding(16)
                .style(panel_style(BG_PANEL_ALT, Some(TEXT_MAIN), Some(16.0))),
            );
        } else {
            for field in output_fields {
                output_items = output_items.push(self.value_box(field.label, field.value));
            }
        }

        let output_panel = container(
            column![
                row![
                    text("Route Output").size(18).color(TEXT_MAIN),
                    container(self.copy_button(output.clone()))
                        .width(Fill)
                        .align_right(Fill),
                ]
                .align_y(Alignment::Center),
                output_items,
            ]
            .spacing(12),
        )
        .padding(18)
        .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(22.0)));

        column![
            header,
            scrollable(output_panel)
                .direction(default_vertical_scroll_direction())
                .style(content_scrollable_style)
                .height(Fill)
        ]
        .spacing(16)
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn preferences_view(&self) -> Element<'_, Message> {
        container(
            scrollable(
                container(
                    column![
                        text("Preferences").size(28).color(TEXT_MAIN),
                        text(
                            "The monitor stores one xmrigcc-proxy HTTP API endpoint, one optional Bearer token, and the poll interval."
                        )
                        .size(15)
                        .color(TEXT_MUTED),
                        self.settings_editor(true),
                    ]
                    .spacing(18),
                )
                .padding(22),
            )
            .direction(default_vertical_scroll_direction())
            .style(content_scrollable_style)
            .width(Fill)
            .height(Fill),
        )
        .width(Fill)
        .height(Fill)
        .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(22.0)))
        .into()
    }

    fn settings_editor(&self, preferences_mode: bool) -> Element<'_, Message> {
        let title = if preferences_mode {
            "Update HTTP API settings and reconnect"
        } else {
            "Connection settings"
        };

        let action_label = if preferences_mode {
            "Save Settings"
        } else {
            "Verify and Continue"
        };

        let section = container(
            column![
                text("HTTP API").size(20).color(TEXT_MAIN),
                row![
                    self.field(
                        "Host",
                        "127.0.0.1",
                        &self.api_host_input,
                        false,
                        Message::UpdateApiHost,
                    ),
                    self.field(
                        "Port",
                        "80",
                        &self.api_port_input,
                        false,
                        Message::UpdateApiPort,
                    ),
                ]
                .spacing(14),
                self.transport_field(
                    "Transport",
                    &self.api_transport_input,
                    Message::UpdateApiTransport,
                ),
                self.field(
                    "Access Token",
                    "Bearer token",
                    &self.api_access_token_input,
                    false,
                    Message::UpdateApiAccessToken,
                ),
                self.field(
                    "Poll Frequency (seconds)",
                    "10",
                    &self.poll_frequency_input,
                    false,
                    Message::UpdatePollFrequency,
                ),
                self.value_box(
                    "Verification",
                    "Save Settings validates GET /1/summary before writing settings.json."
                        .to_string(),
                ),
            ]
            .spacing(14),
        )
        .padding(20)
        .style(panel_style(BG_PANEL_ALT, Some(TEXT_MAIN), Some(20.0)));

        container(
            column![
                text(title).size(22).color(TEXT_MAIN),
                section,
                self.message_panel(),
                button(text(action_label).size(16))
                    .padding([12, 20])
                    .style(move |_theme, status| primary_button_style(status))
                    .on_press(Message::SaveAndConnect),
            ]
            .spacing(16),
        )
        .padding(20)
        .style(panel_style(BG_PANEL_ALT, Some(TEXT_MAIN), Some(20.0)))
        .into()
    }

    fn field(
        &self,
        label: &'static str,
        placeholder: &'static str,
        value: &str,
        secure: bool,
        on_input: fn(String) -> Message,
    ) -> Element<'_, Message> {
        let input = text_input(placeholder, value)
            .on_input(on_input)
            .secure(secure)
            .padding(12)
            .size(16)
            .width(Fill)
            .style(input_style);

        container(
            column![text(label).size(13).color(TEXT_MUTED), input]
                .spacing(8)
                .width(Fill),
        )
        .width(Fill)
        .into()
    }

    fn transport_field(
        &self,
        label: &'static str,
        selected: &str,
        on_selected: fn(String) -> Message,
    ) -> Element<'_, Message> {
        let options = transport_options();
        let current = options
            .iter()
            .find(|option| option.as_str() == selected)
            .cloned();

        container(
            column![
                text(label).size(13).color(TEXT_MUTED),
                pick_list(options, current, on_selected)
                    .placeholder("Select transport")
                    .padding([10, 14])
                    .text_size(15)
                    .style(pick_list_style),
            ]
            .spacing(8),
        )
        .width(Fill)
        .into()
    }

    fn info_card<'a>(
        &'a self,
        label: &'a str,
        value: impl Into<String>,
    ) -> Element<'a, Message> {
        let value = value.into();
        container(
            column![
                text(label).size(13).color(TEXT_MUTED),
                row![
                    container(text(value.clone()).size(22).color(TEXT_MAIN)).width(Fill),
                    self.copy_button(value),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(10),
        )
        .width(Fill)
        .padding(18)
        .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(18.0)))
        .into()
    }

    fn value_box(
        &self,
        label: impl Into<String>,
        value: impl Into<String>,
    ) -> Element<'_, Message> {
        let label = label.into();
        let value = value.into();

        container(
            column![
                text(label).size(13).color(TEXT_MUTED),
                container(
                    row![
                        container(text(value.clone()).size(16).color(TEXT_MAIN)).width(Fill),
                        self.copy_button(value),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([12, 14])
                .style(panel_style(BG_PANEL_ALT, Some(TEXT_MAIN), Some(14.0))),
            ]
            .spacing(8),
        )
        .into()
    }

    fn summary_grid(&self) -> Element<'_, Message> {
        let left = column![
            self.metric_line("Worker ID", self.display_value(self.summary.worker_id.as_deref())),
            self.metric_line("API ID", self.display_value(self.summary.api_id.as_deref())),
            self.metric_line("Kind", self.display_value(self.summary.kind.as_deref())),
            self.metric_line("Uptime", self.display_value(self.summary.uptime.as_deref())),
            self.metric_line("Restricted", self.display_value(self.summary.restricted.as_deref())),
            self.metric_line(
                "Donate Level",
                self.display_value(self.summary.donate_level.as_deref()),
            ),
            self.metric_line("Donated", self.display_value(self.summary.donated.as_deref())),
        ]
        .spacing(10);

        let right = column![
            self.metric_line("Miners Now", self.display_value(self.summary.miners_now.as_deref())),
            self.metric_line("Miners Max", self.display_value(self.summary.miners_max.as_deref())),
            self.metric_line(
                "Upstream Active",
                self.display_value(self.summary.upstream_active.as_deref()),
            ),
            self.metric_line("Rejected", self.display_value(self.summary.rejected.as_deref())),
            self.metric_line("Invalid", self.display_value(self.summary.invalid.as_deref())),
            self.metric_line("Expired", self.display_value(self.summary.expired.as_deref())),
            self.metric_line("Latency", self.display_value(self.summary.latency.as_deref())),
            self.metric_line("Avg Time", self.display_value(self.summary.avg_time.as_deref())),
            self.metric_line(
                "Memory RSS",
                self.display_value(self.summary.memory_rss.as_deref()),
            ),
        ]
        .spacing(10);

        container(
            column![
                row![left, right].spacing(28),
                self.long_value_line(
                    "Features",
                    self.display_value(self.summary.features.as_deref()),
                ),
                self.long_value_line(
                    "Load Average",
                    self.display_value(self.summary.load_average.as_deref()),
                ),
            ]
            .spacing(14),
        )
        .padding(18)
        .style(panel_style(BG_PANEL_ALT, Some(TEXT_MAIN), Some(16.0)))
        .into()
    }

    fn metric_line<'a>(
        &'a self,
        label: &'a str,
        value: impl Into<String>,
    ) -> Element<'a, Message> {
        let value = value.into();
        row![
            text(label).size(13).color(TEXT_MUTED),
            container(text(value.clone()).size(15).color(TEXT_MAIN))
                .width(Fill)
                .align_right(Fill),
            self.copy_button(value),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .into()
    }

    fn long_value_line<'a>(
        &'a self,
        label: &'a str,
        value: impl Into<String>,
    ) -> Element<'a, Message> {
        let value = value.into();

        container(
            column![
                text(label).size(13).color(TEXT_MUTED),
                container(
                    row![
                        container(
                            text(value.clone())
                                .size(13)
                                .color(TEXT_MAIN)
                                .wrapping(iced::widget::text::Wrapping::None),
                        )
                        .width(Fill),
                        self.copy_button(value),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .width(Fill)
                .padding([12, 14])
                .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(14.0))),
            ]
            .spacing(8),
        )
        .width(Fill)
        .into()
    }

    fn message_panel(&self) -> Element<'_, Message> {
        let notice = self.notice.as_deref().map(|message| {
            self.copyable_message_line(message, TEXT_MUTED)
        });
        let error = self
            .error
            .as_deref()
            .map(|message| self.copyable_message_line(message, DANGER));

        let content = match (notice, error) {
            (Some(notice), Some(error)) => column![notice, error].spacing(8),
            (Some(notice), None) => column![notice],
            (None, Some(error)) => column![error],
            (None, None) => column![self.copyable_message_line("No warnings.", TEXT_MUTED)],
        };

        container(content)
            .padding(14)
            .style(panel_style(BG_PANEL, Some(TEXT_MAIN), Some(14.0)))
            .into()
    }

    fn copyable_message_line<'a>(
        &'a self,
        value: &'a str,
        color: Color,
    ) -> Element<'a, Message> {
        row![
            container(text(value).size(14).color(color)).width(Fill),
            self.copy_button(value.to_string()),
        ]
        .spacing(10)
        .align_y(Alignment::Start)
        .into()
    }

    fn copy_button(&self, value: impl Into<String>) -> Element<'_, Message> {
        button(text("Copy").size(12).color(TEXT_MAIN))
            .padding([6, 10])
            .style(secondary_button_style)
            .on_press(Message::CopyToClipboard(value.into()))
            .into()
    }

    fn nav_button(&self, label: &'static str, view: View) -> Element<'_, Message> {
        let active = self.view == view;

        button(
            row![
                text(label)
                    .size(18)
                    .color(if active { TEXT_MAIN } else { TEXT_MUTED }),
                container(text(">").size(16).color(if active { ACCENT } else { TEXT_MUTED }))
                    .width(Fill)
                    .align_right(Fill),
            ]
            .align_y(Alignment::Center),
        )
        .width(Fill)
        .padding([14, 16])
        .style(move |_theme, status| sidebar_button_style(active, status))
        .on_press(Message::SelectView(view))
        .into()
    }

    fn menu_button(
        &self,
        label: &'static str,
        message: Message,
        active: bool,
    ) -> Element<'_, Message> {
        button(text(label).size(14).color(TEXT_MAIN))
            .padding([8, 12])
            .style(move |_theme, status| top_button_style(active, status))
            .on_press(message)
            .into()
    }

    fn refresh_status(&mut self) {
        self.error = None;
        self.notice = None;

        match self.settings_from_inputs() {
            Ok(settings) => match Self::poll_with_settings(&settings, None) {
                Ok(outcome) => self.apply_poll(outcome),
                Err(error) => self.apply_disconnect(error, false),
            },
            Err(error) => self.error = Some(error),
        }
    }

    fn manual_poll_selected_endpoint(&mut self) {
        self.error = None;
        self.notice = None;

        let selected_endpoint = self.selected_endpoint.clone();

        match self.settings_from_inputs() {
            Ok(settings) => match Self::poll_with_settings(&settings, Some(&selected_endpoint)) {
                Ok(outcome) => self.apply_poll(outcome),
                Err(error) => self.apply_disconnect(error, true),
            },
            Err(error) => {
                self.error = Some(error.clone());
                self.selected_output = Some(json!({ "error": error }));
            }
        }
    }

    fn save_and_connect(&mut self) -> Result<(), String> {
        self.error = None;
        self.notice = None;

        let settings = self.settings_from_inputs()?;
        let outcome = Self::poll_with_settings(&settings, None)?;

        settings
            .save()
            .map_err(|error| format!("Failed to save settings.json: {error}"))?;

        self.notice = Some("HTTP API connection verified and settings saved.".into());
        self.apply_poll(outcome);

        Ok(())
    }

    fn connect_with_current_inputs(&mut self) -> Result<(), String> {
        self.error = None;

        let settings = self.settings_from_inputs()?;
        let outcome = Self::poll_with_settings(&settings, None)?;
        self.apply_poll(outcome);

        Ok(())
    }

    fn settings_from_inputs(&self) -> Result<Settings, String> {
        let defaults = Settings::default();
        let api_host = trimmed_or_default(&self.api_host_input, &defaults.api_host);

        let api_port = parse_or_default_u16(
            &self.api_port_input,
            defaults.api_port,
            "HTTP API port must be a valid number between 0 and 65535.",
        )?;

        let poll_frequency_seconds = parse_or_default_u64(
            &self.poll_frequency_input,
            defaults.poll_frequency_seconds,
            "Poll frequency must be a valid number of seconds.",
        )?;

        if poll_frequency_seconds == 0 {
            return Err("Poll frequency must be greater than zero.".into());
        }

        let preferred_endpoint = self
            .selected_endpoint_spec()
            .map(|endpoint| endpoint.path.clone())
            .or_else(|| default_endpoint(&self.api_endpoints))
            .ok_or_else(|| "No safe HTTP API routes were loaded from http-api.output.".to_string())?;

        Ok(Settings {
            api_host,
            api_port,
            api_transport: normalize_transport(&self.api_transport_input, "http"),
            api_access_token: self.api_access_token_input.trim().to_string(),
            poll_frequency_seconds,
            preferred_endpoint,
        })
    }

    fn settings_snapshot(&self) -> Settings {
        self.settings_from_inputs()
            .unwrap_or_else(|_| Settings::default())
    }

    fn safe_endpoint_paths(&self) -> Vec<String> {
        self.api_endpoints
            .iter()
            .filter(|endpoint| endpoint.safe_to_poll)
            .map(|endpoint| endpoint.path.clone())
            .collect()
    }

    fn ensure_selected_endpoint(&mut self) {
        let valid = self
            .api_endpoints
            .iter()
            .any(|endpoint| endpoint.safe_to_poll && endpoint.path == self.selected_endpoint);

        if !valid {
            self.selected_endpoint = default_endpoint(&self.api_endpoints).unwrap_or_default();
        }
    }

    fn selected_endpoint_spec(&self) -> Option<&ApiEndpointSpec> {
        self.api_endpoints
            .iter()
            .find(|endpoint| endpoint.path == self.selected_endpoint)
    }

    fn poll_with_settings(
        settings: &Settings,
        selected_endpoint: Option<&str>,
    ) -> Result<PollOutcome, String> {
        let client = ApiClient::new(settings.api_connection())?;
        let summary_json = client.get_json("/1/summary")?;
        let summary = SummarySnapshot::from_summary(&summary_json);

        let mut captured_output = None;
        let mut notice = Some("GET /1/summary completed successfully.".into());
        let mut error = None;

        if let Some(path) = selected_endpoint {
            if path == "/1/summary" {
                captured_output = Some(summary_json.clone());
                notice = Some("GET /1/summary completed successfully and its payload was captured.".into());
            } else {
                match client.get_json(path) {
                    Ok(value) => {
                        captured_output = Some(value);
                        notice = Some(format!(
                            "GET /1/summary completed successfully and {path} was captured."
                        ));
                    }
                    Err(route_error) => {
                        captured_output = Some(json!({
                            "path": path,
                            "error": route_error.clone(),
                        }));
                        notice = Some(format!(
                            "GET /1/summary completed successfully, but {path} failed."
                        ));
                        error = Some(route_error);
                    }
                }
            }
        }

        Ok(PollOutcome {
            summary,
            summary_json,
            captured_output,
            notice,
            error,
        })
    }

    fn apply_poll(&mut self, outcome: PollOutcome) {
        let polled_at = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        self.connection_status = "Connected".into();
        self.last_poll = polled_at;
        self.summary = outcome.summary;
        self.last_summary_json = Some(outcome.summary_json.clone());
        self.notice = outcome.notice;
        self.error = outcome.error;

        if let Some(output) = outcome.captured_output {
            self.selected_output = Some(output);
        } else if self.selected_endpoint == "/1/summary" || self.selected_output.is_none() {
            self.selected_output = Some(outcome.summary_json);
        }
    }

    fn apply_disconnect(&mut self, error: String, capture_output: bool) {
        self.connection_status = "Disconnected".into();
        self.notice = Some("HTTP API status check failed.".into());
        self.error = Some(error.clone());
        self.summary = SummarySnapshot::default();
        self.last_summary_json = None;

        if capture_output {
            self.selected_output = Some(json!({ "error": error }));
        }
    }

    fn persist_window_size(&mut self, size: Size) {
        let Some(window_state) = WindowState::from_size(size) else {
            return;
        };

        if let Err(error) = window_state.save() {
            if self.error.is_none() {
                self.error = Some(format!("Failed to save window size: {error}"));
            }
        }
    }

    fn selected_output_value(&self) -> Option<&Value> {
        self.selected_output.as_ref().or_else(|| {
            if self.selected_endpoint == "/1/summary" {
                self.last_summary_json.as_ref()
            } else {
                None
            }
        })
    }

    fn selected_output_fields(&self) -> Vec<OutputField> {
        let Some(output) = self.selected_output_value() else {
            return Vec::new();
        };

        output_fields_for_endpoint(&self.selected_endpoint, output)
    }

    fn selected_output_copy_text(&self) -> String {
        let fields = self.selected_output_fields();

        if fields.is_empty() {
            return "No API response has been captured yet. Select a route and press Poll.".into();
        }

        fields
            .into_iter()
            .map(|field| format!("{}: {}", field.label, field.value))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn display_value(&self, value: Option<&str>) -> String {
        value.unwrap_or("Waiting").to_string()
    }
}

fn output_fields_for_endpoint(endpoint: &str, value: &Value) -> Vec<OutputField> {
    match endpoint {
        "/1/miners" => miners_output_fields(value),
        "/1/workers" => workers_output_fields(value),
        _ => {
            let mut fields = Vec::new();
            flatten_json_fields(None, value, &mut fields);
            fields
        }
    }
}

fn miners_output_fields(value: &Value) -> Vec<OutputField> {
    let mut fields = Vec::new();

    if let Some(object) = value.as_object() {
        for (key, entry) in object {
            if key != "miners" && key != "format" {
                flatten_json_fields(Some(key.clone()), entry, &mut fields);
            }
        }

        let format_labels = object
            .get("format")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(render_json_value).collect::<Vec<_>>())
            .unwrap_or_default();

        if let Some(miners) = object.get("miners").and_then(Value::as_array) {
            for (miner_index, miner) in miners.iter().enumerate() {
                if let Some(row) = miner.as_array() {
                    for (field_index, item) in row.iter().enumerate() {
                        let field_name = format_labels
                            .get(field_index)
                            .cloned()
                            .unwrap_or_else(|| format!("field_{field_index}"));
                        fields.push(OutputField {
                            label: format!("miners[{miner_index}].{field_name}"),
                            value: render_json_value(item),
                        });
                    }
                } else {
                    flatten_json_fields(Some(format!("miners[{miner_index}]")), miner, &mut fields);
                }
            }
        }
    } else {
        flatten_json_fields(None, value, &mut fields);
    }

    fields
}

fn workers_output_fields(value: &Value) -> Vec<OutputField> {
    const WORKER_FIELDS: [&str; 13] = [
        "name",
        "ip",
        "connections",
        "accepted",
        "rejected",
        "invalid",
        "hashes",
        "last_hash",
        "hashrate_60s",
        "hashrate_10m",
        "hashrate_1h",
        "hashrate_12h",
        "hashrate_24h",
    ];

    let mut fields = Vec::new();

    if let Some(object) = value.as_object() {
        for (key, entry) in object {
            if key != "workers" {
                flatten_json_fields(Some(key.clone()), entry, &mut fields);
            }
        }

        if let Some(workers) = object.get("workers").and_then(Value::as_array) {
            for (worker_index, worker) in workers.iter().enumerate() {
                if let Some(row) = worker.as_array() {
                    for (field_index, item) in row.iter().enumerate() {
                        let field_name = WORKER_FIELDS.get(field_index).copied().unwrap_or("value");
                        fields.push(OutputField {
                            label: format!("workers[{worker_index}].{field_name}"),
                            value: render_json_value(item),
                        });
                    }
                } else {
                    flatten_json_fields(
                        Some(format!("workers[{worker_index}]")),
                        worker,
                        &mut fields,
                    );
                }
            }
        }
    } else {
        flatten_json_fields(None, value, &mut fields);
    }

    fields
}

fn flatten_json_fields(prefix: Option<String>, value: &Value, fields: &mut Vec<OutputField>) {
    match value {
        Value::Object(map) => {
            if map.is_empty() {
                fields.push(OutputField {
                    label: prefix.unwrap_or_else(|| "value".into()),
                    value: "{}".into(),
                });
                return;
            }

            for (key, entry) in map {
                let label = match &prefix {
                    Some(prefix) => format!("{prefix}.{key}"),
                    None => key.clone(),
                };
                flatten_json_fields(Some(label), entry, fields);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                fields.push(OutputField {
                    label: prefix.unwrap_or_else(|| "value".into()),
                    value: "[]".into(),
                });
                return;
            }

            for (index, entry) in items.iter().enumerate() {
                let label = match &prefix {
                    Some(prefix) => format!("{prefix}[{index}]"),
                    None => format!("[{index}]"),
                };
                flatten_json_fields(Some(label), entry, fields);
            }
        }
        _ => {
            fields.push(OutputField {
                label: prefix.unwrap_or_else(|| "value".into()),
                value: render_json_value(value),
            });
        }
    }
}

impl SummarySnapshot {
    fn from_summary(value: &Value) -> Self {
        Self {
            api_id: value_string(value.get("id")),
            worker_id: value_string(value.get("worker_id")),
            version: value_string(value.get("version")),
            kind: value_string(value.get("kind")),
            mode: value_string(value.get("mode")),
            uptime: value_string(value.get("uptime")),
            restricted: value
                .get("restricted")
                .and_then(Value::as_bool)
                .map(|value| if value { "true" } else { "false" }.to_string()),
            features: value
                .get("features")
                .and_then(Value::as_array)
                .map(|values| join_json_values(values)),
            hashrate_total: value
                .get("hashrate")
                .and_then(|hashrate| hashrate.get("total"))
                .and_then(Value::as_array)
                .map(|values| join_hashrate_values(values)),
            workers: value_string(value.get("workers")),
            miners_now: value
                .get("miners")
                .and_then(|miners| miners.get("now"))
                .map(render_json_value),
            miners_max: value
                .get("miners")
                .and_then(|miners| miners.get("max"))
                .map(render_json_value),
            upstream_active: value
                .get("upstreams")
                .and_then(|upstreams| upstreams.get("active"))
                .map(render_json_value),
            upstream_ratio: value
                .get("upstreams")
                .and_then(|upstreams| upstreams.get("ratio"))
                .map(render_json_value),
            accepted: value
                .get("results")
                .and_then(|results| results.get("accepted"))
                .map(render_json_value),
            rejected: value
                .get("results")
                .and_then(|results| results.get("rejected"))
                .map(render_json_value),
            invalid: value
                .get("results")
                .and_then(|results| results.get("invalid"))
                .map(render_json_value),
            expired: value
                .get("results")
                .and_then(|results| results.get("expired"))
                .map(render_json_value),
            avg_time: value
                .get("results")
                .and_then(|results| results.get("avg_time"))
                .map(render_json_value),
            latency: value
                .get("results")
                .and_then(|results| results.get("latency"))
                .map(render_json_value),
            donate_level: value.get("donate_level").map(render_json_value),
            donated: value.get("donated").map(render_json_value),
            memory_rss: value
                .get("resources")
                .and_then(|resources| resources.get("memory"))
                .and_then(|memory| memory.get("resident_set_memory"))
                .and_then(Value::as_u64)
                .map(format_bytes),
            load_average: value
                .get("resources")
                .and_then(|resources| resources.get("load_average"))
                .and_then(Value::as_array)
                .map(|values| join_json_values(values)),
        }
    }
}

fn value_string(value: Option<&Value>) -> Option<String> {
    value.map(render_json_value)
}

fn join_json_values(values: &[Value]) -> String {
    values
        .iter()
        .map(render_json_value)
        .collect::<Vec<_>>()
        .join(", ")
}

fn join_hashrate_values(values: &[Value]) -> String {
    values
        .iter()
        .map(render_json_value)
        .collect::<Vec<_>>()
        .join(" / ")
}

fn render_json_value(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        _ => value.to_string(),
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes = bytes as f64;

    if bytes >= GB {
        format!("{:.2} GiB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.2} MiB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.2} KiB", bytes / KB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn panel_style(
    background: Color,
    text_color: Option<Color>,
    radius: Option<f32>,
) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| {
        container::Style::default()
            .background(Background::Color(background))
            .color(text_color.unwrap_or(TEXT_MAIN))
            .border(
                Border::default()
                    .rounded(radius.unwrap_or(16.0))
                    .width(1.0)
                    .color(BORDER_SOFT),
            )
            .shadow(Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
                offset: iced::Vector::new(0.0, 6.0),
                blur_radius: 18.0,
            })
    }
}

fn primary_button_style(status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => ACCENT,
        button::Status::Pressed => ACCENT_DIM,
        button::Status::Disabled => BG_PANEL,
        _ => ACCENT_DIM,
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: TEXT_MAIN,
        border: Border::default().rounded(14.0).width(1.0).color(ACCENT),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn secondary_button_style(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => BG_PANEL_ALT,
        button::Status::Pressed => BG_SIDEBAR,
        button::Status::Disabled => BG_PANEL,
        _ => BG_PANEL,
    };

    let border_color = match status {
        button::Status::Hovered => ACCENT_DIM,
        button::Status::Pressed => ACCENT,
        _ => BORDER_SOFT,
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: TEXT_MAIN,
        border: Border::default()
            .rounded(10.0)
            .width(1.0)
            .color(border_color),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn top_button_style(active: bool, status: button::Status) -> button::Style {
    let background = if active {
        BG_PANEL_ALT
    } else {
        match status {
            button::Status::Hovered => BG_PANEL_ALT,
            button::Status::Pressed => BG_SIDEBAR,
            _ => Color::TRANSPARENT,
        }
    };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: TEXT_MAIN,
        border: Border::default()
            .rounded(12.0)
            .width(1.0)
            .color(if active { ACCENT_DIM } else { BORDER_SOFT }),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn sidebar_button_style(active: bool, status: button::Status) -> button::Style {
    let background = if active {
        BG_PANEL_ALT
    } else {
        match status {
            button::Status::Hovered => Color::from_rgb(0.12, 0.12, 0.13),
            button::Status::Pressed => BG_PANEL_ALT,
            _ => Color::TRANSPARENT,
        }
    };

    let border_color = if active { ACCENT_DIM } else { BORDER_SOFT };

    button::Style {
        background: Some(Background::Color(background)),
        text_color: TEXT_MAIN,
        border: Border::default()
            .rounded(14.0)
            .width(if active { 1.2 } else { 1.0 })
            .color(border_color),
        shadow: Shadow::default(),
        snap: false,
    }
}

fn input_style(_theme: &Theme, status: text_input::Status) -> text_input::Style {
    let border_color = match status {
        text_input::Status::Focused { .. } => ACCENT,
        text_input::Status::Hovered => ACCENT_DIM,
        _ => BORDER_SOFT,
    };

    text_input::Style {
        background: Background::Color(BG_PANEL),
        border: Border::default()
            .rounded(12.0)
            .width(1.0)
            .color(border_color),
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT_MAIN,
        selection: ACCENT_DIM,
    }
}

fn pick_list_style(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let border_color = match status {
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => ACCENT_DIM,
        pick_list::Status::Active => BORDER_SOFT,
    };

    pick_list::Style {
        text_color: TEXT_MAIN,
        placeholder_color: TEXT_MUTED,
        handle_color: TEXT_MUTED,
        background: Background::Color(BG_PANEL_ALT),
        border: Border::default()
            .rounded(14.0)
            .width(1.0)
            .color(border_color),
    }
}

fn default_vertical_scroll_direction() -> iced::widget::scrollable::Direction {
    iced::widget::scrollable::Direction::Vertical(
        iced::widget::scrollable::Scrollbar::new()
            .width(12)
            .scroller_width(12)
            .margin(2),
    )
}

fn content_scrollable_style(
    _theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    let scroller_color = match status {
        iced::widget::scrollable::Status::Dragged { .. } => ACCENT,
        iced::widget::scrollable::Status::Hovered { .. } => ACCENT_DIM,
        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.22),
    };

    let rail = iced::widget::scrollable::Rail {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
        border: Border::default()
            .rounded(10.0)
            .width(1.0)
            .color(BORDER_SOFT),
        scroller: iced::widget::scrollable::Scroller {
            background: Background::Color(scroller_color),
            border: Border::default().rounded(10.0),
        },
    };

    iced::widget::scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.03))),
        auto_scroll: iced::widget::scrollable::default(&_theme.clone(), status).auto_scroll,
    }
}

fn transport_options() -> Vec<String> {
    vec!["http".to_string(), "https".to_string()]
}

fn normalize_transport(value: &str, fallback: &str) -> String {
    let lowered = value.trim().to_ascii_lowercase();
    match lowered.as_str() {
        "http" | "https" => lowered,
        _ => fallback.to_string(),
    }
}

fn trimmed_or_default(value: &str, default: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn parse_or_default_u16(value: &str, default: u16, error_message: &str) -> Result<u16, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(default)
    } else {
        trimmed.parse::<u16>().map_err(|_| error_message.to_string())
    }
}

fn parse_or_default_u64(value: &str, default: u64, error_message: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(default)
    } else {
        trimmed.parse::<u64>().map_err(|_| error_message.to_string())
    }
}
