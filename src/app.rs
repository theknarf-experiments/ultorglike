use tui_input::{InputContext, Mode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusPanel {
    Metrics,
    Chart,
    Info,
    Collectors,
}

pub struct App {
    pub input: InputContext,
    pub should_quit: bool,
    pub focus: FocusPanel,
    pub selected: usize,
    pub notice: Option<Notice>,
    pub data_lines: Vec<String>,
}

pub struct Notice {
    pub title: String,
    pub rows: Vec<Vec<String>>,
    pub selected: usize,
}

impl App {
    pub fn new() -> Self {
        let mut input = InputContext::new();

        // Register command handlers
        input.register_handler("quit", |state| {
            state.should_quit = true;
            true
        });
        input.register_handler("help", |_| false);
        input.register_handler("cursor_up", |_| false);
        input.register_handler("cursor_down", |_| false);
        input.register_handler("jump_to_first_row", |_| false);
        input.register_handler("jump_to_last_row", |_| false);
        input.register_handler("pan_left", |_| false);
        input.register_handler("pan_down", |_| false);
        input.register_handler("pan_up", |_| false);
        input.register_handler("pan_right", |_| false);
        input.register_handler("zoom_in", |_| false);
        input.register_handler("zoom_out", |_| false);
        input.register_handler("center_viewport", |_| false);
        input.register_handler("enter_visual_mode", |_| false);
        input.register_handler("enter_insert_mode", |_| false);
        input.register_handler("maps", |state| {
            state.message = Some("Use :map to see mappings".to_string());
            false
        });

        // Generate some mock data
        let data_lines = vec![
            "CPU Usage:          45.2%".to_string(),
            "Memory:            12.4 GB / 32.0 GB".to_string(),
            "Swap:               1.2 GB / 8.0 GB".to_string(),
            "Disk (root):       234.5 GB / 512.0 GB".to_string(),
            "Uptime:            3 days, 14:22:07".to_string(),
            "CPU Temp:           52.3 C".to_string(),
            "GPU Usage:          12.1%".to_string(),
            "Network Up:       1.2 Mbps".to_string(),
            "Network Down:     4.5 Mbps".to_string(),
            "Battery:           87% (Charging)".to_string(),
        ];

        Self {
            input,
            should_quit: false,
            focus: FocusPanel::Metrics,
            selected: 0,
            notice: None,
            data_lines,
        }
    }

    pub fn handle_command(&mut self, command: &str) {
        match command {
            "quit" => {
                self.should_quit = true;
            }
            "help" => {
                if matches!(self.input.mode(), Mode::Help) {
                    self.input.set_mode(Mode::Normal);
                } else {
                    self.input.set_mode(Mode::Help);
                }
            }
            "cursor_up" | "pan_up" => {
                self.selected = self.selected.saturating_sub(1);
            }
            "cursor_down" | "pan_down" => {
                self.selected = self.selected.min(self.data_lines.len().saturating_sub(1));
            }
            "jump_to_first_row" | "jump_top" => {
                self.selected = 0;
            }
            "jump_to_last_row" | "jump_bottom" => {
                self.selected = self.data_lines.len().saturating_sub(1);
            }
            _ => {}
        }
    }

    pub fn notice_rows(&self) -> Option<&Vec<Vec<String>>> {
        self.notice.as_ref().map(|n| &n.rows)
    }

    pub fn notice_title(&self) -> Option<&str> {
        self.notice.as_ref().map(|n| n.title.as_str())
    }

    pub fn has_notice(&self) -> bool {
        self.notice.is_some()
    }

    pub fn clear_notice(&mut self) {
        self.notice = None;
    }
}
