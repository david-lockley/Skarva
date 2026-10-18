#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod appearance;
mod attachments;
mod brand;
mod credentials;
mod gmail;
mod jev;
mod llm;
mod memory;
mod model;
mod office;
mod search;
mod storage;

use std::{
    collections::{HashSet, VecDeque},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Id, Pos2, Rect, RichText, Sense, Stroke,
    StrokeKind, Vec2,
};
use egui_material_icons::icons::*;
use model::{BlockCategory, Canvas, Edge, Node, NodeKind, Project};
use storage::Storage;
use zeroize::{Zeroize, Zeroizing};

const BG: Color32 = Color32::from_rgb(20, 22, 23);
const PANEL: Color32 = Color32::from_rgb(27, 29, 31);
const CARD: Color32 = Color32::from_rgb(35, 38, 40);
const LINE: Color32 = Color32::from_rgb(65, 69, 72);
const TEXT: Color32 = Color32::from_rgb(237, 239, 238);
const MUTED: Color32 = Color32::from_rgb(144, 150, 152);
const ACCENT: Color32 = Color32::from_rgb(56, 108, 125);
const ACCENT_DETAIL: Color32 = Color32::from_rgb(128, 182, 196);

fn primary_button(label: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(RichText::new(label.into()).color(BG).strong())
        .fill(TEXT)
        .corner_radius(8)
}

fn kind_color(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Start => Color32::from_rgb(139, 191, 164),
        NodeKind::Decision | NodeKind::YesNo => Color32::from_rgb(205, 171, 119),
        NodeKind::Copilot => Color32::from_rgb(156, 148, 224),
        NodeKind::OpenAI => Color32::from_rgb(155, 195, 175),
        NodeKind::Teams => Color32::from_rgb(149, 157, 229),
        NodeKind::Outlook => Color32::from_rgb(118, 171, 223),
        NodeKind::Excel => Color32::from_rgb(111, 187, 139),
        NodeKind::Word => Color32::from_rgb(113, 155, 216),
        NodeKind::PowerPoint => Color32::from_rgb(212, 130, 100),
        NodeKind::Note => Color32::from_rgb(196, 174, 120),
        NodeKind::End => Color32::from_rgb(175, 180, 183),
        NodeKind::GmailInbox | NodeKind::GmailReply => Color32::from_rgb(218, 126, 108),
        NodeKind::WebSearch => Color32::from_rgb(112, 173, 185),
        NodeKind::LocalFile => Color32::from_rgb(179, 162, 204),
        NodeKind::Recall | NodeKind::Remember => Color32::from_rgb(168, 181, 130),
    }
}

fn kind_icon(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Start => ICON_PLAY_ARROW,
        NodeKind::Decision => ICON_ALT_ROUTE,
        NodeKind::YesNo => ICON_RULE,
        NodeKind::Copilot => ICON_SMART_TOY,
        NodeKind::OpenAI => ICON_PSYCHOLOGY,
        NodeKind::Teams => ICON_FORUM,
        NodeKind::Outlook => ICON_MAIL,
        NodeKind::Excel => ICON_TABLE_CHART,
        NodeKind::Word => ICON_ARTICLE,
        NodeKind::PowerPoint => ICON_SLIDESHOW,
        NodeKind::Note => ICON_NOTE_ALT,
        NodeKind::End => ICON_STOP,
        NodeKind::GmailInbox => ICON_INBOX,
        NodeKind::GmailReply => ICON_REPLY,
        NodeKind::WebSearch => ICON_TRAVEL_EXPLORE,
        NodeKind::LocalFile => ICON_DESCRIPTION,
        NodeKind::Recall => ICON_MANAGE_SEARCH,
        NodeKind::Remember => ICON_BOOKMARK_ADD,
    }
}

fn category_icon(category: BlockCategory) -> &'static str {
    match category {
        BlockCategory::Flow => ICON_ACCOUNT_TREE,
        BlockCategory::Messages => ICON_CHAT_BUBBLE,
        BlockCategory::Ai => ICON_AUTO_AWESOME,
        BlockCategory::Office => ICON_GRID_VIEW,
        BlockCategory::Files => ICON_FOLDER_OPEN,
    }
}

#[derive(Default)]
struct BlockPickerState {
    expanded: [bool; 5],
    search: String,
    focus_search: bool,
}

impl BlockPickerState {
    fn with_search_focused() -> Self {
        Self {
            focus_search: true,
            ..Self::default()
        }
    }
}

fn category_index(category: BlockCategory) -> usize {
    match category {
        BlockCategory::Flow => 0,
        BlockCategory::Messages => 1,
        BlockCategory::Ai => 2,
        BlockCategory::Office => 3,
        BlockCategory::Files => 4,
    }
}

fn picker_row(
    ui: &mut egui::Ui,
    width: f32,
    icon: (&str, Color32),
    label: &str,
    indent: f32,
    active: bool,
    trailing: Option<&str>,
) -> bool {
    let (icon, color) = icon;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if active || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 2.0)),
            6.0,
            if active {
                Color32::from_rgb(41, 48, 54)
            } else {
                Color32::from_rgb(43, 47, 49)
            },
        );
    }
    ui.painter().text(
        Pos2::new(rect.left() + 12.0 + indent, rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        FontId::proportional(17.0),
        color,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 39.0 + indent, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        FontId::proportional(12.5),
        TEXT,
    );
    if let Some(trailing) = trailing {
        ui.painter().text(
            Pos2::new(rect.right() - 11.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            trailing,
            FontId::proportional(16.0),
            MUTED,
        );
    }
    response.clicked()
}

fn block_picker(
    ui: &mut egui::Ui,
    state: &mut BlockPickerState,
    width: f32,
    accepts_input: bool,
    max_height: f32,
) -> Option<NodeKind> {
    let search = egui::Frame::new()
        .fill(BG)
        .corner_radius(7)
        .stroke(Stroke::new(1.0_f32, LINE))
        .inner_margin(egui::Margin::symmetric(9, 0))
        .show(ui, |ui| {
            ui.set_width(width - 18.0);
            ui.set_height(32.0);
            ui.with_layout(egui::Layout::left_to_right(Align::Center), |ui| {
                ui.label(RichText::new(ICON_SEARCH).size(16.0).color(MUTED));
                ui.add_sized(
                    [width - 48.0, 32.0],
                    egui::TextEdit::singleline(&mut state.search)
                        .frame(false)
                        .vertical_align(Align::Center)
                        .hint_text("Search blocks"),
                )
            })
            .inner
        })
        .inner;
    if state.focus_search {
        search.request_focus();
        state.focus_search = false;
    }
    ui.add_space(7.0);
    let query = state.search.trim().to_lowercase();
    let mut chosen = None;
    egui::ScrollArea::vertical()
        .max_height(max_height.max(80.0))
        .show(ui, |ui| {
            let mut found = false;
            for category in BlockCategory::ALL {
                let matches = NodeKind::ALL
                    .into_iter()
                    .filter(|kind| {
                        (!accepts_input || kind.accepts_input())
                            && kind.category() == category
                            && (query.is_empty()
                                || kind.label().to_lowercase().contains(&query)
                                || kind.picker_detail().to_lowercase().contains(&query)
                                || category.label().to_lowercase().contains(&query))
                    })
                    .collect::<Vec<_>>();
                if matches.is_empty() {
                    continue;
                }
                found = true;
                let index = category_index(category);
                let expanded = state.expanded[index];
                if query.is_empty() {
                    let chevron = if expanded {
                        ICON_EXPAND_MORE
                    } else {
                        ICON_CHEVRON_RIGHT
                    };
                    if picker_row(
                        ui,
                        width,
                        (category_icon(category), MUTED),
                        category.label(),
                        0.0,
                        expanded,
                        Some(chevron),
                    ) {
                        state.expanded[index] = !expanded;
                    }
                } else {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(category.label().to_uppercase())
                            .size(10.0)
                            .strong()
                            .color(MUTED),
                    );
                }
                if !query.is_empty() || expanded {
                    for kind in matches {
                        if picker_row(
                            ui,
                            width,
                            (kind_icon(kind), kind_color(kind)),
                            kind.label(),
                            if query.is_empty() { 16.0 } else { 0.0 },
                            false,
                            None,
                        ) {
                            chosen = Some(kind);
                        }
                    }
                }
                ui.add_space(2.0);
            }
            if !found {
                ui.label(RichText::new("No matching blocks").size(12.0).color(MUTED));
            }
        });
    chosen
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum RunMode {
    #[default]
    Jev,
    Email,
}

enum ActionResult {
    Mail(Option<gmail::Mail>),
    Draft(String),
    Artifact(office::Artifact),
    Sent(String),
    Research(String),
    File(attachments::Attachment),
    Recalled(Option<String>),
    Remembered,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActionKind {
    Fetch,
    Draft,
    Office,
    Send,
    Search,
    File,
    Memory,
}

struct PendingAction {
    kind: ActionKind,
    receiver: Receiver<Result<ActionResult, String>>,
}

struct PendingRoute {
    edges: Vec<Edge>,
    labels: Vec<String>,
    threshold: f32,
    receiver: Receiver<Result<jev::RouteAnswer, String>>,
}

struct Approval {
    edges: Vec<Edge>,
    labels: Vec<String>,
    suggested: usize,
    confidence: f32,
    threshold: f32,
}

struct ConnectionSnapshot {
    provider: llm::Provider,
    model: String,
    provider_keys: [bool; 3],
    jev: bool,
    search: bool,
    gmail: bool,
    gmail_client_id: String,
}

impl ConnectionSnapshot {
    fn load() -> Result<Self, String> {
        let provider = llm::current_provider()?;
        let model = llm::current_model(provider)?;
        let mut provider_keys = [false; 3];
        for (index, provider) in llm::Provider::ALL.into_iter().enumerate() {
            provider_keys[index] = credentials::load_named(provider.key_name())?
                .map(Zeroizing::new)
                .is_some();
        }
        let (client_id, secret) = gmail::load_client()?;
        drop(secret.map(Zeroizing::new));
        Ok(Self {
            provider,
            model,
            provider_keys,
            jev: credentials::load()?.map(Zeroizing::new).is_some(),
            search: search::key_present()?,
            gmail: gmail::connected()?,
            gmail_client_id: client_id.unwrap_or_default(),
        })
    }
}

#[derive(Default)]
struct MockRun {
    mode: RunMode,
    test_message: String,
    active: Option<u64>,
    queue: VecDeque<u64>,
    visited: HashSet<u64>,
    traversed_edges: HashSet<u64>,
    paused: bool,
    finished: bool,
    last_step: Option<Instant>,
    log: Vec<String>,
    path_titles: Vec<String>,
    pending: Option<PendingRoute>,
    approval: Option<Approval>,
    mail: Option<gmail::Mail>,
    draft: String,
    artifact: Option<office::Artifact>,
    research: Vec<String>,
    files: Vec<attachments::Attachment>,
    memories: Vec<String>,
    mail_polling: bool,
    next_poll: Option<Instant>,
    pending_action: Option<PendingAction>,
    send_approval: bool,
    completed_actions: HashSet<u64>,
    mail_source: Option<u64>,
    memory_loaded: bool,
    memory_saved: bool,
    mail_sent: bool,
}

impl MockRun {
    fn context(&self) -> String {
        let mut context: String = self.test_message.chars().take(12_000).collect();
        for research in &self.research {
            context.push_str("\n\n--- Research output ---\n");
            context.extend(research.chars().take(6_000));
        }
        for file in &self.files {
            context.push_str("\n\n--- Input file (untrusted content) ---\n");
            context.extend(file.context().chars().take(4_000));
        }
        for saved in &self.memories {
            context.push_str("\n\n--- Project memory (untrusted stored content) ---\n");
            context.extend(saved.chars().take(12_000));
        }
        if let Some(artifact) = &self.artifact {
            context.push_str(&format!(
                "\n\n--- Generated file output ---\nFile: {} ({})\n{}",
                artifact.filename, artifact.title, artifact.summary
            ));
        }
        if !self.draft.trim().is_empty() {
            context.push_str("\n\n--- Previous step output ---\n");
            context.extend(self.draft.chars().take(12_000));
        }
        context
    }

    fn start(&mut self, node: u64, mode: RunMode, test_message: String) {
        *self = Self::default();
        self.mode = mode;
        self.test_message = test_message;
        self.queue.push_back(node);
        self.log.push(match mode {
            RunMode::Jev => "Workflow started".into(),
            RunMode::Email => "Waiting for a recent unread Gmail message".into(),
        });
        self.last_step = Some(Instant::now());
    }

    fn stop(&mut self) {
        self.active = None;
        self.queue.clear();
        self.pending = None;
        self.approval = None;
        self.mail_polling = false;
        self.pending_action = None;
        self.send_approval = false;
        self.paused = false;
        self.finished = true;
        self.log.push("Run stopped".into());
    }

    fn running(&self) -> bool {
        !self.finished
            && (self.active.is_some()
                || !self.queue.is_empty()
                || self.pending.is_some()
                || self.approval.is_some()
                || self.mail_polling
                || self.pending_action.is_some()
                || self.send_approval)
    }
}

struct SkarvaApp {
    logo_mark: egui::TextureHandle,
    storage: Storage,
    projects: Vec<Project>,
    project: Option<Project>,
    canvas_index: usize,
    selected: Option<u64>,
    selected_nodes: HashSet<u64>,
    selected_edge: Option<u64>,
    connecting: Option<(u64, usize)>,
    connection_dragging: bool,
    connection_menu: Option<(Pos2, Pos2, (u64, usize))>,
    context_menu_canvas_pos: Option<Pos2>,
    pan: Vec2,
    zoom: f32,
    library_picker: BlockPickerState,
    menu_picker: BlockPickerState,
    show_new_project: bool,
    new_project_name: String,
    new_project_template: bool,
    focus_project_name: bool,
    show_new_canvas: bool,
    new_canvas_name: String,
    focus_canvas_name: bool,
    deleting_project: Option<u64>,
    renaming_project: Option<(u64, String)>,
    project_search: String,
    integration_tab: usize,
    fit_canvas: bool,
    show_run_log: bool,
    run: MockRun,
    key_draft: Zeroizing<String>,
    key_present: Option<bool>,
    show_integrations: bool,
    llm_provider: llm::Provider,
    llm_model: String,
    llm_key_draft: Zeroizing<String>,
    search_key_draft: Zeroizing<String>,
    search_key_present: Option<bool>,
    gmail_client_id_draft: String,
    gmail_client_secret_draft: Zeroizing<String>,
    gmail_connected: Option<bool>,
    gmail_connect_receiver: Option<Receiver<Result<(), String>>>,
    seen_mail: HashSet<String>,
    integrations_loaded: bool,
    connection_preferences_loaded: bool,
    connection_receiver: Option<Receiver<Result<ConnectionSnapshot, String>>>,
    start_after_connections: bool,
    provider_keys: [bool; 3],
    llm_key_present: Option<bool>,
    toast: Option<(String, Instant)>,
    dirty_at: Option<Instant>,
    pan_dragging: bool,
    node_drag: Option<(Pos2, Vec<(u64, Vec2)>)>,
    marquee: Option<(Pos2, Pos2, HashSet<u64>)>,
    block_menu_rect: Option<Rect>,
}

impl SkarvaApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self::with_storage(&cc.egui_ctx, Storage::new())
    }

    fn with_storage(ctx: &egui::Context, storage: Storage) -> Self {
        egui_material_icons::initialize(ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = PANEL;
        visuals.window_fill = PANEL;
        visuals.extreme_bg_color = BG;
        visuals.widgets.noninteractive.bg_fill = CARD;
        visuals.widgets.inactive.bg_fill = CARD;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 52, 54);
        visuals.widgets.active.bg_fill = Color32::from_rgb(56, 64, 70);
        visuals.widgets.inactive.fg_stroke.color = TEXT;
        visuals.widgets.hovered.fg_stroke.color = TEXT;
        visuals.widgets.active.fg_stroke.color = TEXT;
        visuals.selection.bg_fill = Color32::from_rgb(47, 75, 89);
        visuals.selection.stroke.color = ACCENT_DETAIL;
        ctx.set_visuals(visuals);
        ctx.style_mut(|style| {
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
            style.spacing.button_padding = Vec2::new(11.0, 7.0);
        });
        let projects = storage.list();
        Self {
            logo_mark: appearance::load_mark(ctx),
            storage,
            projects,
            project: None,
            canvas_index: 0,
            selected: None,
            selected_nodes: HashSet::new(),
            selected_edge: None,
            connecting: None,
            connection_dragging: false,
            connection_menu: None,
            context_menu_canvas_pos: None,
            pan: Vec2::ZERO,
            zoom: 1.0,
            library_picker: BlockPickerState::default(),
            menu_picker: BlockPickerState::default(),
            show_new_project: false,
            new_project_name: String::new(),
            new_project_template: false,
            focus_project_name: true,
            show_new_canvas: false,
            new_canvas_name: String::new(),
            focus_canvas_name: true,
            deleting_project: None,
            renaming_project: None,
            project_search: String::new(),
            integration_tab: 0,
            fit_canvas: false,
            show_run_log: false,
            run: MockRun::default(),
            key_draft: Zeroizing::new(String::new()),
            key_present: None,
            show_integrations: false,
            llm_provider: llm::Provider::default(),
            llm_model: llm::Provider::default().default_model().into(),
            llm_key_draft: Zeroizing::new(String::new()),
            search_key_draft: Zeroizing::new(String::new()),
            search_key_present: None,
            gmail_client_id_draft: String::new(),
            gmail_client_secret_draft: Zeroizing::new(String::new()),
            gmail_connected: None,
            gmail_connect_receiver: None,
            seen_mail: HashSet::new(),
            integrations_loaded: false,
            connection_preferences_loaded: false,
            connection_receiver: None,
            start_after_connections: false,
            provider_keys: [false; 3],
            llm_key_present: None,
            toast: None,
            dirty_at: None,
            pan_dragging: false,
            node_drag: None,
            marquee: None,
            block_menu_rect: None,
        }
    }

    fn canvas(&self) -> Option<&Canvas> {
        self.project.as_ref()?.canvases.get(self.canvas_index)
    }
    fn canvas_mut(&mut self) -> Option<&mut Canvas> {
        self.project.as_mut()?.canvases.get_mut(self.canvas_index)
    }
    fn changed(&mut self) {
        self.dirty_at = Some(Instant::now());
    }
    fn message(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn save(&mut self) {
        if let Some(project) = &self.project
            && let Err(error) = self.storage.save(project)
        {
            self.message(format!("Could not save: {error}"));
            self.dirty_at = Some(Instant::now() + Duration::from_secs(5));
            return;
        }
        self.dirty_at = None;
    }

    fn open_project(&mut self, project: Project) {
        self.start_after_connections = false;
        self.project = Some(project);
        self.canvas_index = 0;
        self.selected = None;
        self.selected_nodes.clear();
        self.selected_edge = None;
        self.connecting = None;
        self.connection_dragging = false;
        self.connection_menu = None;
        self.context_menu_canvas_pos = None;
        self.library_picker = BlockPickerState::default();
        self.menu_picker = BlockPickerState::default();
        self.pan = Vec2::ZERO;
        self.zoom = 1.0;
        self.run = MockRun::default();
        self.node_drag = None;
        self.marquee = None;
        self.fit_canvas = true;
    }

    fn create_project(&mut self) {
        let name = self.new_project_name.trim();
        if name.is_empty() {
            return;
        }
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let project = if self.new_project_template {
            Project::text_workflow(id, name)
        } else {
            Project::new(id, name)
        };
        self.open_project(project);
        self.save();
        self.show_new_project = false;
        self.new_project_name.clear();
    }

    fn import_project(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Skarva project", &["json"])
            .pick_file()
        else {
            return;
        };
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        match self.storage.import(&path, id) {
            Ok(project) => {
                self.open_project(project);
                self.message("Project imported. Review its instructions and local file paths before running.");
            }
            Err(error) => self.message(format!("Could not import project: {error}")),
        }
    }

    fn export_project(&mut self) {
        let Some(project) = &self.project else { return };
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Skarva project", &["json"])
            .set_file_name("workflow.json")
            .save_file()
        else {
            return;
        };
        let result = serde_json::to_vec_pretty(project)
            .map_err(|e| e.to_string())
            .and_then(|bytes| std::fs::write(path, bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => self.message(
                "Project exported. Service keys and conversation history are not included.",
            ),
            Err(error) => self.message(format!("Could not export project: {error}")),
        }
    }

    fn dialog_open(&self) -> bool {
        self.show_integrations
            || self.show_new_project
            || self.show_new_canvas
            || self.renaming_project.is_some()
            || self.deleting_project.is_some()
    }

    fn add_node(&mut self, kind: NodeKind, center: Option<Pos2>) -> Option<u64> {
        if self.run.running() {
            return None;
        }
        let pos = if let Some(center) = center {
            Vec2::new(
                center.x / self.zoom + self.pan.x - 124.0,
                center.y / self.zoom + self.pan.y - 40.0,
            )
        } else {
            let anchor = self.canvas().and_then(|canvas| {
                self.selected
                    .and_then(|id| canvas.node(id))
                    .or_else(|| canvas.nodes.last())
            });
            let mut position = anchor
                .map(|node| Vec2::new(node.x + 340.0, node.y))
                .unwrap_or(self.pan - Vec2::new(124.0, 40.0));
            while self.canvas().is_some_and(|canvas| {
                canvas.nodes.iter().any(|node| {
                    (node.x - position.x).abs() < 280.0 && (node.y - position.y).abs() < 160.0
                })
            }) {
                position.y += 180.0;
            }
            position
        };
        let project = self.project.as_mut()?;
        let id = project.allocate_id();
        if let Some(canvas) = project.canvases.get_mut(self.canvas_index) {
            canvas.nodes.push(Node::new(id, kind, pos.x, pos.y));
            self.selected = Some(id);
            self.selected_nodes.clear();
            self.selected_nodes.insert(id);
            if center.is_none() {
                self.fit_canvas = true;
            }
            self.changed();
            return Some(id);
        }
        None
    }

    fn delete_selected(&mut self) {
        if self.run.running() {
            return;
        }
        if self.selected_nodes.is_empty() {
            return;
        }
        let ids = std::mem::take(&mut self.selected_nodes);
        self.selected = None;
        if let Some(canvas) = self.canvas_mut() {
            canvas.nodes.retain(|n| !ids.contains(&n.id));
            canvas
                .edges
                .retain(|e| !ids.contains(&e.from) && !ids.contains(&e.to));
            self.changed();
        }
    }

    fn delete_selected_edge(&mut self) {
        if self.run.running() {
            return;
        }
        let Some(id) = self.selected_edge.take() else {
            return;
        };
        if let Some(canvas) = self.canvas_mut() {
            canvas.edges.retain(|edge| edge.id != id);
            self.changed();
        }
    }

    fn duplicate_selected(&mut self) {
        if self.run.running() {
            return;
        }
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let copied = project.duplicate_nodes(self.canvas_index, &self.selected_nodes);
        if !copied.is_empty() {
            self.selected = copied.first().copied();
            self.selected_nodes = copied.into_iter().collect();
            self.changed();
        }
    }

    fn connect_to(&mut self, to: u64) {
        if self.run.running() {
            return;
        }
        let Some((from, output)) = self.connecting.take() else {
            return;
        };
        if from == to {
            return;
        }
        let Some(project) = self.project.as_mut() else {
            return;
        };
        let id = project.allocate_id();
        let canvas = &mut project.canvases[self.canvas_index];
        if !canvas
            .edges
            .iter()
            .any(|e| e.from == from && e.output == output && e.to == to)
        {
            canvas.edges.push(Edge {
                id,
                from,
                output,
                to,
            });
            self.changed();
        }
    }

    fn tick_run(&mut self, ctx: &egui::Context) {
        if !self.run.running() || self.run.paused {
            return;
        }
        if self.run.approval.is_some() || self.run.send_approval {
            return;
        }
        if let Some(pending) = &self.run.pending_action {
            match pending.receiver.try_recv() {
                Ok(result) => {
                    let kind = self.run.pending_action.take().unwrap().kind;
                    match result {
                        Ok(ActionResult::Mail(Some(mail))) => {
                            if !self.seen_mail.insert(mail.id.clone()) {
                                self.run.next_poll = Some(Instant::now() + Duration::from_secs(15));
                            } else {
                                self.run
                                    .log
                                    .push(format!("Received Gmail message: {}", mail.subject));
                                if !mail.attachments.is_empty() {
                                    let readable = mail
                                        .attachments
                                        .iter()
                                        .filter(|file| file.text.is_some())
                                        .count();
                                    self.run.log.push(format!(
                                        "Read {readable}/{} incoming attachments; contents available to Jev and AI steps",
                                        mail.attachments.len()
                                    ));
                                    for file in &mail.attachments {
                                        if let Some(note) = &file.note {
                                            self.run.log.push(format!("{}: {note}", file.filename));
                                        }
                                    }
                                }
                                self.run.test_message = mail.message_context();
                                self.run.files = mail.attachments.clone();
                                self.run.mail = Some(mail);
                                self.run.mail_polling = false;
                                if let Some(source) = self.run.mail_source {
                                    self.run.queue.push_back(source);
                                }
                            }
                        }
                        Ok(ActionResult::Mail(None)) => {
                            self.run.next_poll = Some(Instant::now() + Duration::from_secs(15));
                        }
                        Ok(ActionResult::Draft(draft)) => {
                            self.run.draft = draft;
                            self.run.log.push("AI model produced text".into());
                            if let Some(id) = self.run.active {
                                self.run.completed_actions.insert(id);
                                self.run.queue.push_front(id);
                            }
                        }
                        Ok(ActionResult::Artifact(artifact)) => {
                            self.run.log.push(format!(
                                "Created {} · saved to {}",
                                artifact.filename,
                                artifact.path.display()
                            ));
                            self.run.artifact = Some(artifact);
                            self.run.draft.clear();
                            if let Some(id) = self.run.active {
                                self.run.completed_actions.insert(id);
                                self.run.queue.push_front(id);
                            }
                        }
                        Ok(ActionResult::Research(research)) => {
                            self.run
                                .log
                                .push("Web research added to the run context".into());
                            self.run.research.push(research);
                            if let Some(id) = self.run.active {
                                self.run.completed_actions.insert(id);
                                self.run.queue.push_front(id);
                            }
                        }
                        Ok(ActionResult::File(file)) => {
                            self.run
                                .log
                                .push(format!("Read local file: {}", file.filename));
                            if let Some(note) = &file.note {
                                self.run.log.push(format!("{}: {note}", file.filename));
                            }
                            self.run.files.push(file);
                            if let Some(id) = self.run.active {
                                self.run.completed_actions.insert(id);
                                self.run.queue.push_front(id);
                            }
                        }
                        Ok(ActionResult::Recalled(value)) => {
                            if let Some(value) = value {
                                self.run.memories.push(value);
                                self.run
                                    .log
                                    .push("Previous conversation context loaded".into());
                            }
                        }
                        Ok(ActionResult::Remembered) => {
                            self.run.log.push("Context remembered for next time".into());
                        }
                        Ok(ActionResult::Sent(message_id)) => {
                            self.run.mail_sent = true;
                            self.run
                                .log
                                .push(format!("Gmail reply sent · message {message_id}"));
                            if let Some(id) = self.run.active {
                                self.run.completed_actions.insert(id);
                                self.run.queue.push_front(id);
                            }
                        }
                        Err(error) => {
                            if kind == ActionKind::Memory {
                                self.run
                                    .log
                                    .push(format!("Could not update remembered context: {error}"));
                                ctx.request_repaint();
                                return;
                            }
                            self.run.stop();
                            self.run.log.push(format!(
                                "{}: {error}",
                                match kind {
                                    ActionKind::Fetch => "Gmail read failed",
                                    ActionKind::Draft => "AI draft failed",
                                    ActionKind::Office => "Office document failed",
                                    ActionKind::Send => "Gmail send failed",
                                    ActionKind::Search => "Web research failed",
                                    ActionKind::File => "Local file failed",
                                    ActionKind::Memory => "Project memory failed",
                                }
                            ));
                        }
                    }
                    self.run.last_step = Some(Instant::now());
                    ctx.request_repaint();
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
                Err(TryRecvError::Disconnected) => {
                    self.run.stop();
                    self.run
                        .log
                        .push("Integration request ended unexpectedly".into());
                }
            }
            return;
        }
        if self.run.mail_polling && self.run.queue.is_empty() {
            if self.run.next_poll.is_some_and(|at| at > Instant::now()) {
                ctx.request_repaint_after(Duration::from_secs(1));
                return;
            }
            let seen = self.seen_mail.clone();
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let _ = sender.send(gmail::next_unread(&seen).map(ActionResult::Mail));
            });
            self.run.pending_action = Some(PendingAction {
                kind: ActionKind::Fetch,
                receiver,
            });
            self.run.next_poll = None;
            self.run.log.push("Checking Gmail inbox…".into());
            ctx.request_repaint_after(Duration::from_millis(50));
            return;
        }
        if let Some(pending) = self.run.pending.as_ref() {
            match pending.receiver.try_recv() {
                Ok(result) => {
                    let pending = self.run.pending.take().expect("pending route exists");
                    match result {
                        Ok(answer) => {
                            let Some(index) = pending
                                .edges
                                .iter()
                                .position(|edge| answer.choice == format!("edge_{}", edge.id))
                            else {
                                self.run.stop();
                                self.run.log.push("Jev returned an unavailable path".into());
                                return;
                            };
                            self.run.log.push(format!(
                                "Jev chose {} · {:.0}% probability · {:.0}% confidence",
                                pending.labels[index],
                                answer.probability * 100.0,
                                answer.confidence * 100.0
                            ));
                            if answer.confidence < pending.threshold {
                                self.run.log.push("Waiting for human approval".into());
                                self.run.approval = Some(Approval {
                                    edges: pending.edges,
                                    labels: pending.labels,
                                    suggested: index,
                                    confidence: answer.confidence,
                                    threshold: pending.threshold,
                                });
                            } else {
                                self.follow_run_edge(pending.edges[index].clone());
                            }
                        }
                        Err(error) => {
                            self.run.stop();
                            self.run.log.push(error);
                        }
                    }
                    self.run.last_step = Some(Instant::now());
                    ctx.request_repaint();
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(40)),
                Err(TryRecvError::Disconnected) => {
                    self.run.stop();
                    self.run.log.push("Jev request ended unexpectedly".into());
                }
            }
            return;
        }
        if self
            .run
            .last_step
            .is_some_and(|t| t.elapsed() < Duration::from_millis(650))
        {
            ctx.request_repaint_after(Duration::from_millis(40));
            return;
        }
        if !self.run.memory_loaded {
            self.run.memory_loaded = true;
            if let Some(path) = self.memory_location() {
                let key = self.automatic_memory_key();
                let (sender, receiver) = mpsc::channel();
                thread::spawn(move || {
                    let _ = sender.send(memory::recall(&path, &key).map(ActionResult::Recalled));
                });
                self.run.pending_action = Some(PendingAction {
                    kind: ActionKind::Memory,
                    receiver,
                });
                ctx.request_repaint_after(Duration::from_millis(40));
                return;
            }
        }
        let node = self.run.queue.pop_front();
        if let Some(id) = node {
            let step = self.canvas().and_then(|canvas| {
                let node = canvas.node(id)?.clone();
                let outgoing = canvas
                    .edges
                    .iter()
                    .filter(|e| e.from == id)
                    .cloned()
                    .collect::<Vec<_>>();
                let labels = outgoing
                    .iter()
                    .map(|edge| {
                        let target = canvas.node(edge.to);
                        if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) {
                            format!(
                                "{} / {}",
                                node.output_label(edge.output),
                                target.map(|n| n.title.as_str()).unwrap_or("Unknown")
                            )
                        } else {
                            target
                                .map(|n| n.title.clone())
                                .unwrap_or_else(|| "Unknown".into())
                        }
                    })
                    .collect::<Vec<_>>();
                let options = outgoing
                    .iter()
                    .map(|edge| {
                        let target = canvas.node(edge.to);
                        jev::OptionDescription {
                            id: format!("edge_{}", edge.id),
                            description: if matches!(
                                node.kind,
                                NodeKind::Decision | NodeKind::YesNo
                            ) {
                                format!(
                                    "Route '{}': continue to {} ({})",
                                    node.output_label(edge.output),
                                    target.map(|n| n.title.as_str()).unwrap_or("Unknown"),
                                    target.map(|n| n.subtitle.as_str()).unwrap_or("")
                                )
                            } else {
                                format!(
                                    "Continue to {} block '{}' ({})",
                                    target.map(|n| n.kind.label()).unwrap_or("Unknown"),
                                    target.map(|n| n.title.as_str()).unwrap_or("Unknown"),
                                    target.map(|n| n.subtitle.as_str()).unwrap_or("")
                                )
                            },
                        }
                    })
                    .collect::<Vec<_>>();
                Some((node, outgoing, labels, options))
            });
            if let Some((node, outgoing, labels, options)) = step {
                self.run.active = Some(id);
                if self.run.visited.insert(id) {
                    self.run.path_titles.push(node.title.clone());
                    self.run.log.push(format!("Visited {}", node.title));
                }
                if node.kind == NodeKind::WebSearch && !self.run.completed_actions.contains(&id) {
                    let provider = self.llm_provider;
                    let model = self.llm_model.clone();
                    let instructions = node.prompt.clone();
                    let context = self.run.context();
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(
                            search::research(provider, &model, &instructions, &context)
                                .map(ActionResult::Research),
                        );
                    });
                    self.run.pending_action = Some(PendingAction {
                        kind: ActionKind::Search,
                        receiver,
                    });
                    self.run.log.push("Researching the web…".into());
                    ctx.request_repaint_after(Duration::from_millis(50));
                    return;
                }
                if node.kind == NodeKind::LocalFile && !self.run.completed_actions.contains(&id) {
                    let path = std::path::PathBuf::from(node.prompt.trim());
                    if node.prompt.trim().is_empty() {
                        self.run.stop();
                        self.run
                            .log
                            .push("Choose a file for the Local file block".into());
                        return;
                    }
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(attachments::load(&path).map(ActionResult::File));
                    });
                    self.run.pending_action = Some(PendingAction {
                        kind: ActionKind::File,
                        receiver,
                    });
                    self.run.log.push("Reading local file…".into());
                    ctx.request_repaint_after(Duration::from_millis(50));
                    return;
                }
                if matches!(
                    node.kind,
                    NodeKind::Word | NodeKind::Excel | NodeKind::PowerPoint
                ) && !self.run.completed_actions.contains(&id)
                {
                    let provider = self.llm_provider;
                    let model = self.llm_model.clone();
                    let instructions = node.prompt.clone();
                    let email = self.run.context();
                    let directory = self
                        .storage
                        .directory
                        .parent()
                        .unwrap_or(&self.storage.directory)
                        .join("generated");
                    let kind = node.kind;
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(
                            office::generate(
                                kind,
                                provider,
                                &model,
                                &instructions,
                                &email,
                                &directory,
                            )
                            .map(ActionResult::Artifact),
                        );
                    });
                    self.run.pending_action = Some(PendingAction {
                        kind: ActionKind::Office,
                        receiver,
                    });
                    self.run.log.push(format!(
                        "Creating {} with {}…",
                        node.kind.label(),
                        provider.label()
                    ));
                    ctx.request_repaint_after(Duration::from_millis(50));
                    return;
                }
                if node.kind == NodeKind::OpenAI && !self.run.completed_actions.contains(&id) {
                    let provider = self.llm_provider;
                    let model = self.llm_model.clone();
                    let instructions = if node.prompt.trim().is_empty() {
                        "Respond to the incoming request clearly and concisely. Do not invent facts."
                    } else {
                        node.prompt.as_str()
                    }
                    .to_owned();
                    let mut email = self.run.context();
                    if let Some(artifact) = &self.run.artifact {
                        email.push_str(&format!(
                            "\n\nThe generated {} file named {} will be attached automatically. Write an email that introduces it without repeating the entire file.",
                            artifact.title, artifact.filename
                        ));
                    }
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(
                            llm::generate(provider, &model, &instructions, &email)
                                .map(ActionResult::Draft),
                        );
                    });
                    self.run.pending_action = Some(PendingAction {
                        kind: ActionKind::Draft,
                        receiver,
                    });
                    self.run
                        .log
                        .push(format!("Asking {} to draft…", provider.label()));
                    ctx.request_repaint_after(Duration::from_millis(50));
                    return;
                }
                if self.run.mode == RunMode::Email
                    && node.kind == NodeKind::GmailReply
                    && !self.run.completed_actions.contains(&id)
                {
                    if self.run.draft.trim().is_empty()
                        && let Some(artifact) = &self.run.artifact
                    {
                        let name = &artifact.title;
                        self.run.draft = format!(
                            "I've attached {name} as requested. Let me know if you would like any changes."
                        );
                    }
                    if self.run.draft.trim().is_empty() {
                        self.run.stop();
                        self.run
                            .log
                            .push("Gmail reply needs an AI model block before it".into());
                    } else {
                        self.run.send_approval = true;
                        self.run
                            .log
                            .push("Review the reply and approve sending".into());
                    }
                    return;
                }
                if matches!(self.run.mode, RunMode::Jev | RunMode::Email) && outgoing.len() > 1 {
                    let request = jev::RouteRequest {
                        test_message: self.run.context(),
                        current_block: format!("{} ({})", node.title, node.kind.label()),
                        question: if node.kind == NodeKind::YesNo {
                            format!(
                                "First answer this yes or no question: {} Then choose the best connected destination under that answer.",
                                node.routing_question()
                            )
                        } else {
                            node.routing_question()
                        },
                        path: self.run.path_titles.clone(),
                        options,
                    };
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(jev::route(request));
                    });
                    self.run.pending = Some(PendingRoute {
                        edges: outgoing,
                        labels,
                        threshold: node.threshold,
                        receiver,
                    });
                    self.run.log.push("Asking Jev to choose a path…".into());
                    ctx.request_repaint_after(Duration::from_millis(40));
                    return;
                }
                let chosen = outgoing.first().cloned();
                if let Some(edge) = chosen {
                    self.follow_run_edge(edge);
                }
            }
        } else {
            if !self.run.memory_saved && (self.run.mode != RunMode::Email || self.run.mail_sent) {
                self.run.memory_saved = true;
                if let Some(path) = self.memory_location() {
                    let key = self.automatic_memory_key();
                    let output = self
                        .run
                        .artifact
                        .as_ref()
                        .map(|f| f.summary.as_str())
                        .unwrap_or(self.run.draft.as_str());
                    let value = format!(
                        "Previous request:\n{}\n\nPrevious result:\n{}",
                        self.run.test_message, output
                    );
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(
                            memory::remember(&path, &key, &value)
                                .map(|()| ActionResult::Remembered),
                        );
                    });
                    self.run.pending_action = Some(PendingAction {
                        kind: ActionKind::Memory,
                        receiver,
                    });
                    ctx.request_repaint_after(Duration::from_millis(40));
                    return;
                }
            }
            self.run.active = None;
            self.run.finished = true;
            self.run.log.push("Run complete".into());
        }
        self.run.last_step = Some(Instant::now());
        ctx.request_repaint_after(Duration::from_millis(40));
    }

    fn follow_run_edge(&mut self, edge: Edge) {
        self.run.traversed_edges.insert(edge.id);
        if !self.run.visited.contains(&edge.to) {
            self.run.queue.push_back(edge.to);
        }
    }

    fn approve_run_edge(&mut self, index: usize) {
        let Some(approval) = self.run.approval.take() else {
            return;
        };
        if let Some(edge) = approval.edges.get(index) {
            self.run
                .log
                .push(format!("Human chose {}", approval.labels[index]));
            self.follow_run_edge(edge.clone());
            self.run.last_step = Some(Instant::now());
        }
    }

    fn approve_send(&mut self) {
        if !self.run.send_approval {
            return;
        }
        let Some(mail) = self.run.mail.clone() else {
            self.run.stop();
            self.run
                .log
                .push("No Gmail message is attached to this run".into());
            return;
        };
        let draft = self.run.draft.clone();
        let attachment = self.run.artifact.clone();
        self.run.send_approval = false;
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(
                gmail::send_reply(&mail, &draft, attachment.as_ref()).map(ActionResult::Sent),
            );
        });
        self.run.pending_action = Some(PendingAction {
            kind: ActionKind::Send,
            receiver,
        });
        self.run.log.push("Sending approved Gmail reply…".into());
    }

    fn open_generated_file(&mut self) {
        let Some(file) = &self.run.artifact else {
            return;
        };
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&file.path).spawn();
        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer")
            .arg(&file.path)
            .spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let result = std::process::Command::new("xdg-open")
            .arg(&file.path)
            .spawn();
        if let Err(error) = result {
            self.message(format!("Could not open generated file: {error}"));
        }
    }

    fn memory_location(&self) -> Option<std::path::PathBuf> {
        let project = self.project.as_ref()?;
        Some(
            self.storage
                .directory
                .parent()?
                .join("memory")
                .join(format!("{}.json", project.id)),
        )
    }

    fn automatic_memory_key(&self) -> String {
        let canvas = self.canvas().map(|c| c.id).unwrap_or(0);
        if let Some(mail) = &self.run.mail {
            format!("canvas:{canvas}:thread:{}", mail.thread_id)
        } else {
            format!("canvas:{canvas}:manual")
        }
    }

    fn load_connection_preferences(&mut self) {
        if self.connection_receiver.is_none() {
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                let _ = sender.send(ConnectionSnapshot::load());
            });
            self.connection_receiver = Some(receiver);
        }
    }

    fn poll_connections(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.connection_receiver else {
            return;
        };
        match receiver.try_recv() {
            Ok(Ok(snapshot)) => {
                self.connection_receiver = None;
                self.llm_provider = snapshot.provider;
                self.llm_model = snapshot.model;
                self.provider_keys = snapshot.provider_keys;
                self.llm_key_present = Some(
                    self.provider_keys[llm::Provider::ALL
                        .iter()
                        .position(|p| *p == self.llm_provider)
                        .unwrap_or(0)],
                );
                self.key_present = Some(snapshot.jev);
                self.search_key_present = Some(snapshot.search);
                self.gmail_connected = Some(snapshot.gmail);
                self.gmail_client_id_draft = snapshot.gmail_client_id;
                self.connection_preferences_loaded = true;
                self.integrations_loaded = true;
                if self.start_after_connections {
                    self.start_after_connections = false;
                    self.start_run();
                }
            }
            Ok(Err(error)) => {
                self.connection_receiver = None;
                self.start_after_connections = false;
                self.integrations_loaded = true;
                self.show_integrations = true;
                self.message(format!("Could not load saved connections: {error}"));
            }
            Err(TryRecvError::Disconnected) => {
                self.connection_receiver = None;
                self.start_after_connections = false;
                self.integrations_loaded = true;
                self.message("Could not load saved connections");
            }
            Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(60)),
        }
    }

    fn start_run(&mut self) {
        if self.run.running() {
            self.message("Reset or finish the current run before starting another");
            return;
        }
        let Some(canvas) = self.canvas() else { return };
        if let Err(error) = canvas.validate() {
            self.message(error);
            return;
        }
        let starts: Vec<_> = canvas
            .nodes
            .iter()
            .filter(|n| matches!(n.kind, NodeKind::Start | NodeKind::GmailInbox))
            .collect();
        let source = self
            .selected
            .and_then(|id| starts.iter().find(|n| n.id == id).copied())
            .or_else(|| (starts.len() == 1).then(|| starts[0]));
        let Some(source) = source else {
            self.message(if starts.is_empty() {
                "Add a Play or Email received block to start your workflow"
            } else {
                "Select the Play or Email received block you want to run"
            });
            return;
        };
        let start = source.id;
        let mode = if source.kind == NodeKind::GmailInbox {
            RunMode::Email
        } else {
            RunMode::Jev
        };
        let input = source.prompt.clone();
        let mut reachable = HashSet::new();
        let mut pending = vec![start];
        while let Some(id) = pending.pop() {
            if reachable.insert(id) {
                pending.extend(canvas.edges.iter().filter(|e| e.from == id).map(|e| e.to));
            }
        }
        let nodes: Vec<_> = canvas
            .nodes
            .iter()
            .filter(|n| reachable.contains(&n.id))
            .collect();
        let needs_services = nodes.iter().any(|n| {
            matches!(
                n.kind,
                NodeKind::GmailInbox
                    | NodeKind::GmailReply
                    | NodeKind::OpenAI
                    | NodeKind::Word
                    | NodeKind::Excel
                    | NodeKind::PowerPoint
                    | NodeKind::WebSearch
            ) || canvas.edges.iter().filter(|e| e.from == n.id).count() > 1
        });
        if let Some(node) = nodes.iter().find(|n| {
            matches!(
                n.kind,
                NodeKind::Teams | NodeKind::Outlook | NodeKind::Copilot
            )
        }) {
            self.message(format!(
                "{} is not connected to a service yet. Replace it with an available block.",
                node.title
            ));
            return;
        }
        if mode != RunMode::Email && nodes.iter().any(|n| n.kind == NodeKind::GmailReply) {
            self.message("Start with Email received to reply to an email");
            return;
        }
        if let Some(node) = nodes.iter().find(|n| {
            n.kind == NodeKind::YesNo
                && (0..2).any(|output| {
                    !canvas
                        .edges
                        .iter()
                        .any(|e| e.from == n.id && e.output == output)
                })
        }) {
            self.message(format!(
                "Connect both Yes and No on '{}'. Use an End block for a path that should stop.",
                node.title
            ));
            return;
        }
        if needs_services && !self.connection_preferences_loaded {
            self.start_after_connections = true;
            self.load_connection_preferences();
            self.message("Loading saved connections…");
            return;
        }
        let missing = if mode == RunMode::Email && self.gmail_connected != Some(true) {
            Some((3, "Connect Gmail to run this workflow"))
        } else if nodes
            .iter()
            .any(|n| canvas.edges.iter().filter(|e| e.from == n.id).count() > 1)
            && self.key_present != Some(true)
        {
            Some((1, "Connect Jev to choose between paths"))
        } else if nodes.iter().any(|n| {
            matches!(
                n.kind,
                NodeKind::OpenAI
                    | NodeKind::Word
                    | NodeKind::Excel
                    | NodeKind::PowerPoint
                    | NodeKind::WebSearch
            )
        }) && self.llm_key_present != Some(true)
        {
            Some((0, "Connect an AI provider to run these blocks"))
        } else if nodes.iter().any(|n| n.kind == NodeKind::WebSearch)
            && self.search_key_present != Some(true)
        {
            Some((2, "Connect Tavily to research the web"))
        } else {
            None
        };
        if let Some((tab, message)) = missing {
            self.integration_tab = tab;
            self.show_integrations = true;
            self.message(message);
            return;
        }
        self.connecting = None;
        self.connection_dragging = false;
        self.connection_menu = None;
        self.node_drag = None;
        self.marquee = None;
        self.run.start(start, mode, input);
        if mode == RunMode::Email {
            self.run.queue.clear();
            self.run.mail_polling = true;
            self.run.mail_source = Some(start);
        }
        self.show_run_log = true;
    }

    fn project_browser(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("workspace_navigation")
            .exact_width(200.0)
            .resizable(false)
            .show(ctx, |ui| {
                if self.dialog_open() {
                    ui.disable();
                }
                ui.add_space(28.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.add(
                        egui::Image::new(&self.logo_mark).fit_to_exact_size(Vec2::new(32.0, 22.3)),
                    );
                    ui.label(RichText::new(brand::NAME).size(23.0).strong().color(TEXT));
                });
                ui.add_space(42.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    ui.add_sized(
                        [170.0, 38.0],
                        egui::Button::new(format!("{}   Projects", ICON_FOLDER_OPEN))
                            .fill(CARD)
                            .corner_radius(7),
                    );
                });
                ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(18.0);
                    ui.label(
                        RichText::new(format!("Version {} · Preview", env!("CARGO_PKG_VERSION")))
                            .size(10.0)
                            .color(MUTED),
                    );
                    ui.hyperlink_to(
                        "Help & setup",
                        "https://github.com/david-lockley/skarva/blob/main/docs/SETUP.md",
                    );
                    if ui
                        .add_sized(
                            [180.0, 38.0],
                            egui::Button::new(format!("{}   Integrations", ICON_TUNE)).frame(false),
                        )
                        .clicked()
                    {
                        self.show_integrations = true;
                    }
                });
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG).inner_margin(32.0))
            .show(ctx, |ui| {
                if self.dialog_open() { ui.disable(); }
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Projects").size(28.0).strong());
                        ui.label(
                            RichText::new(brand::TAGLINE)
                                .size(13.0)
                                .color(MUTED),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_sized(
                                [135.0, 38.0],
                                primary_button(format!("{}  New project", ICON_ADD)),
                            )
                            .clicked()
                        {
                            self.show_new_project = true;
                        }
                        if ui.add_sized([90.0, 38.0], egui::Button::new("Import…").frame(false)).clicked() {
                            self.import_project();
                        }
                    });
                });
                ui.add_space(28.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(ICON_SEARCH).size(18.0).color(MUTED));
                    ui.add_sized(
                        [260.0, 32.0],
                        egui::TextEdit::singleline(&mut self.project_search)
                            .hint_text("Search projects")
                            .margin(Vec2::new(10.0, 7.0)),
                    );
                });
                ui.add_space(22.0);
                let query = self.project_search.trim().to_lowercase();
                let projects: Vec<_> = self
                    .projects
                    .iter()
                    .filter(|p| p.name.to_lowercase().contains(&query))
                    .cloned()
                    .collect();
                if projects.is_empty() {
                    ui.add_space(75.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(ICON_ACCOUNT_TREE).size(38.0).color(MUTED));
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(if query.is_empty() {
                                "Make room for your next idea"
                            } else {
                                "No matching projects"
                            })
                            .size(20.0)
                            .strong(),
                        );
                        ui.label(
                            RichText::new(if query.is_empty() {
                                "Create a project and connect your first two blocks."
                            } else {
                                "Try a different name."
                            })
                            .color(MUTED),
                        );
                        if query.is_empty() {
                            ui.add_space(12.0);
                            if ui.add(primary_button("Create your first project")).clicked() {
                                self.show_new_project = true;
                            }
                            ui.add_space(10.0);
                            ui.label(RichText::new("Start with a blank canvas or a ready-to-connect text workflow.").size(12.0).color(MUTED));
                            ui.label(RichText::new("Connect only the services your workflow needs.").size(12.0).color(MUTED));
                        }
                    });
                } else {
                    let columns = ((ui.available_width() / 285.0) as usize).clamp(1, 4);
                    let width = ((ui.available_width() - (columns - 1) as f32 * 18.0)
                        / columns as f32)
                        .min(360.0);
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("project_cards")
                            .spacing([18.0, 18.0])
                            .show(ui, |ui| {
                                for (index, project) in projects.iter().enumerate() {
                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::new(width, 202.0),
                                        Sense::click(),
                                    );
                                    let painter = ui.painter().with_clip_rect(rect);
                                    painter.rect_filled(rect, 10, CARD);
                                    painter.rect_stroke(
                                        rect,
                                        10,
                                        Stroke::new(
                                            1.0_f32,
                                            if response.hovered() {
                                                Color32::from_rgb(99, 110, 108)
                                            } else {
                                                LINE
                                            },
                                        ),
                                        StrokeKind::Inside,
                                    );
                                    let preview = Rect::from_min_max(
                                        rect.min + Vec2::splat(1.0),
                                        Pos2::new(rect.right() - 1.0, rect.top() + 130.0),
                                    );
                                    painter.rect_filled(preview, 9, PANEL);
                                    if let Some(canvas) = project.canvases.first() {
                                        if canvas.nodes.is_empty() {
                                            painter.text(
                                                preview.center(),
                                                egui::Align2::CENTER_CENTER,
                                                ICON_ACCOUNT_TREE,
                                                FontId::proportional(28.0),
                                                LINE,
                                            );
                                        } else {
                                            let mut bounds = Rect::NOTHING;
                                            for node in &canvas.nodes {
                                                bounds.extend_with(Pos2::new(node.x, node.y));
                                                bounds.extend_with(Pos2::new(
                                                    node.x + 248.0,
                                                    node.y + 100.0,
                                                ));
                                            }
                                            let scale = ((preview.width() - 42.0)
                                                / bounds.width().max(1.0))
                                            .min(
                                                (preview.height() - 32.0)
                                                    / bounds.height().max(1.0),
                                            )
                                            .min(0.7);
                                            let position = |n: &Node| {
                                                preview.center()
                                                    + (Vec2::new(n.x + 124.0, n.y + 45.0)
                                                        - bounds.center().to_vec2())
                                                        * scale
                                            };
                                            for edge in &canvas.edges {
                                                if let (Some(a), Some(b)) =
                                                    (canvas.node(edge.from), canvas.node(edge.to))
                                                {
                                                    painter.line_segment(
                                                        [position(a), position(b)],
                                                        Stroke::new(1.0_f32, LINE),
                                                    );
                                                }
                                            }
                                            for node in &canvas.nodes {
                                                let r = Rect::from_center_size(
                                                    position(node),
                                                    Vec2::new(
                                                        (248.0 * scale).max(7.0),
                                                        (80.0 * scale).max(4.0),
                                                    ),
                                                );
                                                painter.rect_filled(r, 2, CARD);
                                                painter.rect_stroke(
                                                    r,
                                                    2,
                                                    Stroke::new(
                                                        1.0_f32,
                                                        kind_color(node.kind).gamma_multiply(0.7),
                                                    ),
                                                    StrokeKind::Inside,
                                                );
                                            }
                                        }
                                    }
                                    let title_clip = Rect::from_min_max(
                                        rect.min + Vec2::new(16.0, 140.0),
                                        Pos2::new(rect.right() - 38.0, rect.bottom()),
                                    );
                                    painter.with_clip_rect(title_clip).text(
                                        rect.min + Vec2::new(16.0, 151.0),
                                        egui::Align2::LEFT_CENTER,
                                        &project.name,
                                        FontId::proportional(15.0),
                                        TEXT,
                                    );
                                    let count: usize =
                                        project.canvases.iter().map(|c| c.nodes.len()).sum();
                                    painter.text(
                                        rect.min + Vec2::new(16.0, 178.0),
                                        egui::Align2::LEFT_CENTER,
                                        format!(
                                            "{} canvas{}  ·  {count} blocks",
                                            project.canvases.len(),
                                            if project.canvases.len() == 1 {
                                                ""
                                            } else {
                                                "es"
                                            }
                                        ),
                                        FontId::proportional(11.0),
                                        MUTED,
                                    );
                                    let menu_rect = Rect::from_min_size(
                                        Pos2::new(rect.right() - 36.0, rect.top() + 140.0),
                                        Vec2::new(30.0, 30.0),
                                    );
                                    {
                                        let mut menu_ui = ui
                                            .new_child(egui::UiBuilder::new().max_rect(menu_rect));
                                        menu_ui.menu_button(ICON_MORE_HORIZ, |ui| {
                                            if ui.button("Rename").clicked() {
                                                self.renaming_project =
                                                    Some((project.id, project.name.clone()));
                                                ui.close();
                                            }
                                            if ui.button("Delete…").clicked() {
                                                self.deleting_project = Some(project.id);
                                                ui.close();
                                            }
                                        });
                                    }
                                    response.context_menu(|ui| {
                                        if ui.button("Rename").clicked() {
                                            self.renaming_project =
                                                Some((project.id, project.name.clone()));
                                            ui.close();
                                        }
                                        if ui.button("Delete…").clicked() {
                                            self.deleting_project = Some(project.id);
                                            ui.close();
                                        }
                                    });
                                    if response.clicked()
                                        && !response
                                            .interact_pointer_pos()
                                            .is_some_and(|p| menu_rect.contains(p))
                                    {
                                        self.open_project(project.clone());
                                    }
                                    if (index + 1) % columns == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                }
            });
    }

    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .exact_height(66.0)
            .show(ctx, |ui| {
                if self.dialog_open() {
                    ui.disable();
                }
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(ICON_ARROW_BACK).size(18.0).color(MUTED),
                            )
                            .frame(false),
                        )
                        .on_hover_text("Projects")
                        .clicked()
                    {
                        if self
                            .run
                            .pending_action
                            .as_ref()
                            .is_some_and(|a| a.kind == ActionKind::Send)
                        {
                            self.message("Wait for the email to finish sending");
                        } else {
                            self.save();
                            self.projects = self.storage.list();
                            self.project = None;
                            self.start_after_connections = false;
                            self.run = MockRun::default();
                            return;
                        }
                    }
                    ui.add_space(10.0);
                    ui.add(
                        egui::Image::new(&self.logo_mark).fit_to_exact_size(Vec2::new(26.0, 18.2)),
                    )
                    .on_hover_text(brand::NAME);
                    if let Some(project) = &self.project
                        && ui
                            .add(
                                egui::Button::new(RichText::new(&project.name).size(15.0).strong())
                                    .frame(false),
                            )
                            .on_hover_text("Rename project")
                            .clicked()
                    {
                        self.renaming_project = Some((project.id, project.name.clone()));
                    }
                    ui.menu_button(ICON_MORE_HORIZ, |ui| {
                        if ui.button("Export project…").clicked() {
                            self.export_project();
                            ui.close();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(14.0);
                        let running = self.run.running();
                        let (label, icon) = if self.run.approval.is_some() || self.run.send_approval
                        {
                            ("Review", ICON_FACT_CHECK)
                        } else if running {
                            if self.run.paused {
                                ("Resume", ICON_PLAY_ARROW)
                            } else {
                                ("Pause", ICON_PAUSE)
                            }
                        } else {
                            ("Play", ICON_PLAY_ARROW)
                        };
                        if ui
                            .add_sized([108.0, 38.0], primary_button(format!("{icon}  {label}")))
                            .clicked()
                        {
                            if self.run.approval.is_some() || self.run.send_approval {
                                self.show_run_log = true;
                            } else if running {
                                self.run.paused = !self.run.paused;
                            } else {
                                self.start_run();
                            }
                        }
                        if (running || self.run.finished)
                            && ui
                                .add_sized(
                                    [38.0, 38.0],
                                    egui::Button::new(ICON_RESTART_ALT).frame(false),
                                )
                                .on_hover_text("Reset run")
                                .clicked()
                        {
                            if self
                                .run
                                .pending_action
                                .as_ref()
                                .is_some_and(|a| a.kind == ActionKind::Send)
                            {
                                self.message("Wait for the email to finish sending");
                            } else {
                                self.run = MockRun::default();
                                self.start_after_connections = false;
                            }
                        }
                        ui.add_space(8.0);
                        if ui
                            .add_sized(
                                [126.0, 38.0],
                                egui::Button::new(format!("{}  Integrations", ICON_TUNE))
                                    .fill(CARD)
                                    .corner_radius(8),
                            )
                            .clicked()
                        {
                            self.show_integrations = true;
                        }
                    });
                });
            });
        egui::TopBottomPanel::top("tabs")
            .exact_height(39.0)
            .show(ctx, |ui| {
                if self.dialog_open() {
                    ui.disable();
                }
                ui.horizontal_centered(|ui| {
                    ui.add_space(15.0);
                    let tabs = self
                        .project
                        .as_ref()
                        .map(|p| {
                            p.canvases
                                .iter()
                                .map(|c| c.name.clone())
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    for (index, name) in tabs.into_iter().enumerate() {
                        let active = self.canvas_index == index;
                        let text = RichText::new(format!("  {name}  "))
                            .size(12.0)
                            .color(if active { TEXT } else { MUTED });
                        if ui
                            .add(
                                egui::Button::new(text)
                                    .fill(if active { CARD } else { PANEL })
                                    .frame(active)
                                    .corner_radius(6),
                            )
                            .clicked()
                        {
                            if index == self.canvas_index {
                                continue;
                            }
                            if self
                                .run
                                .pending_action
                                .as_ref()
                                .is_some_and(|action| action.kind == ActionKind::Send)
                            {
                                self.message(
                                    "Wait for Gmail to finish sending before switching canvases",
                                );
                                continue;
                            }
                            self.canvas_index = index;
                            self.start_after_connections = false;
                            self.selected = None;
                            self.selected_nodes.clear();
                            self.selected_edge = None;
                            self.connecting = None;
                            self.connection_dragging = false;
                            self.connection_menu = None;
                            self.context_menu_canvas_pos = None;
                            self.menu_picker = BlockPickerState::default();
                            self.run = MockRun::default();
                            self.pan = Vec2::ZERO;
                            self.zoom = 1.0;
                            self.fit_canvas = true;
                        }
                    }
                    if ui
                        .add_enabled(
                            !self.run.running(),
                            egui::Button::new(RichText::new(ICON_ADD).size(18.0))
                                .fill(PANEL)
                                .frame(false),
                        )
                        .on_hover_text("New canvas")
                        .clicked()
                    {
                        self.show_new_canvas = true;
                    }
                });
            });
    }

    fn library(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("library")
            .exact_width(238.0)
            .resizable(false)
            .show(ctx, |ui| {
                if self.dialog_open() || self.run.running() {
                    ui.disable();
                }
                ui.add_space(17.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new("BLOCKS").size(11.0).strong().color(MUTED));
                });
                ui.add_space(13.0);
                ui.separator();
                ui.add_space(12.0);

                let picker_height = (ui.available_height() - 88.0).max(180.0);
                let chosen =
                    block_picker(ui, &mut self.library_picker, 220.0, false, picker_height);
                if let Some(kind) = chosen {
                    self.add_node(kind, None);
                }
                ui.with_layout(egui::Layout::bottom_up(Align::LEFT), |ui| {
                    ui.add_space(13.0);
                    ui.horizontal(|ui| {
                        ui.add_space(15.0);
                        ui.label(
                            RichText::new("Click to add · right-click canvas")
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                });
            });
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("inspector")
            .exact_width(286.0)
            .resizable(false)
            .show(ctx, |ui| {
                if self.dialog_open() || self.run.running() { ui.disable(); }
                ui.add_space(17.0);
                ui.horizontal(|ui| {
                    ui.add_space(15.0);
                    ui.label(RichText::new("PROPERTIES").size(11.0).strong().color(MUTED));
                });
                ui.add_space(16.0);
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("inspector_scroll")
                    .show(ui, |ui| {
                if let Some(edge_id) = self.selected_edge {
                    let connection = self.canvas().and_then(|canvas| {
                        let edge = canvas.edges.iter().find(|edge| edge.id == edge_id)?;
                        Some((canvas.node(edge.from)?.title.clone(), canvas.node(edge.to)?.title.clone()))
                    });
                    if let Some((from, to)) = connection {
                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new("CONNECTION").size(11.0).strong().color(MUTED));
                        });
                        ui.add_space(12.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new(format!("{from}  {ICON_ARROW_FORWARD}  {to}")).size(13.0).color(TEXT));
                        });
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            if ui.button("Remove connection").clicked() {
                                self.delete_selected_edge();
                            }
                        });
                    } else {
                        self.selected_edge = None;
                    }
                    return;
                }
                let Some(id) = self.selected else {
                    ui.add_space(28.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Nothing selected").size(14.0).color(TEXT));
                        ui.add_space(5.0);
                        ui.label(
                            RichText::new("Select a block to edit its details.")
                                .size(12.0)
                                .color(MUTED),
                        );
                    });
                    return;
                };
                let Some(node) = self.canvas().and_then(|c| c.node(id)).cloned() else {
                    self.selected = None;
                    return;
                };
                if self.selected_nodes.len() > 1 {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new(format!("{} BLOCKS SELECTED · drag to move together", self.selected_nodes.len()))
                            .size(11.0).color(ACCENT_DETAIL));
                    });
                }
                if node.kind == NodeKind::Decision
                    && node.routes.len() == 2
                    && node.routes[0].eq_ignore_ascii_case("yes")
                    && node.routes[1].eq_ignore_ascii_case("no")
                {
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        if ui.small_button("Convert to Yes / No block").clicked()
                            && let Some(existing) = self.canvas_mut().and_then(|c| c.node_mut(id)) {
                                existing.kind = NodeKind::YesNo;
                                existing.routes.clear();
                                self.changed();
                            }
                    });
                }
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new(kind_icon(node.kind))
                            .size(23.0)
                            .color(kind_color(node.kind)),
                    );
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(node.kind.label())
                                .size(14.0)
                                .strong()
                                .color(TEXT),
                        );
                        ui.label(RichText::new(if matches!(node.kind, NodeKind::LocalFile | NodeKind::Recall | NodeKind::Remember) {
                            "LOCAL BLOCK"
                        } else if matches!(node.kind, NodeKind::GmailInbox | NodeKind::GmailReply | NodeKind::OpenAI | NodeKind::Word | NodeKind::Excel | NodeKind::PowerPoint | NodeKind::WebSearch) {
                            "CONNECTED WORKFLOW"
                        } else {
                            "WORKFLOW BLOCK"
                        }).size(10.0).color(MUTED));
                    });
                });
                ui.add_space(24.0);
                let mut changed = false;
                let mut remove_last_route = false;
                let branch_targets = self.canvas().map(|canvas| {
                    canvas.edges.iter().filter(|edge| edge.from == id)
                        .filter_map(|edge| canvas.node(edge.to).map(|node| node.title.clone()))
                        .collect::<Vec<_>>()
                }).unwrap_or_default();
                if let Some(node) = self.canvas_mut().and_then(|c| c.node_mut(id)) {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new("Name").size(12.0).color(MUTED));
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        changed |= ui
                            .add_sized([258.0, 31.0], egui::TextEdit::singleline(&mut node.title))
                            .changed();
                    });
                    if matches!(node.kind, NodeKind::Start | NodeKind::OpenAI | NodeKind::Word | NodeKind::Excel | NodeKind::PowerPoint | NodeKind::WebSearch) {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| { ui.add_space(14.0); ui.label(RichText::new(if node.kind == NodeKind::Start { "STARTING MESSAGE" } else if node.kind == NodeKind::OpenAI { "INSTRUCTIONS" } else if node.kind == NodeKind::WebSearch { "RESEARCH FOCUS" } else { "FILE INSTRUCTIONS" }).size(11.0).strong().color(MUTED)); });
                        ui.horizontal(|ui| { ui.add_space(12.0); changed |= ui.add_sized([258.0, 116.0],
                            egui::TextEdit::multiline(&mut node.prompt).hint_text(if node.kind == NodeKind::Start { "What would you like this workflow to do?" } else if node.kind == NodeKind::OpenAI { "How should the model respond?" } else if node.kind == NodeKind::WebSearch { "What should be researched?" } else { "What should this file contain?" }))
                            .changed(); });
                    }
                    if node.kind == NodeKind::LocalFile {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| { ui.add_space(14.0); ui.label(RichText::new("SOURCE FILE").size(11.0).strong().color(MUTED)); });
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            if ui.button("Choose file…").clicked()
                                && let Some(path) = rfd::FileDialog::new()
                                    .add_filter("Readable files", &["txt", "md", "csv", "tsv", "json", "pdf", "docx", "xlsx", "pptx"])
                                    .pick_file() {
                                    node.prompt = path.to_string_lossy().into_owned();
                                    changed = true;
                                }
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.add_sized([252.0, 44.0], egui::Label::new(
                                RichText::new(if node.prompt.is_empty() { "No file selected" } else { &node.prompt })
                                    .size(11.0).color(MUTED)).wrap());
                        });
                    }
                    if matches!(node.kind, NodeKind::Recall | NodeKind::Remember) {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| { ui.add_space(14.0); ui.label(RichText::new("MEMORY KEY").size(11.0).strong().color(MUTED)); });
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            changed |= ui.add_sized([258.0, 31.0], egui::TextEdit::singleline(&mut node.prompt)
                                .hint_text("{sender} or a fixed key")).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.add_sized([252.0, 42.0], egui::Label::new(
                                RichText::new(if node.kind == NodeKind::Remember {
                                    "Stores the latest generated file summary, AI draft, or incoming message locally."
                                } else {
                                    "Loads a saved value into later AI and Jev steps."
                                }).size(11.0).color(MUTED)).wrap());
                        });
                    }
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new("Description").size(12.0).color(MUTED));
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        changed |= ui
                            .add_sized(
                                [258.0, 31.0],
                                egui::TextEdit::singleline(&mut node.subtitle),
                            )
                            .changed();
                    });
                    if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) || branch_targets.len() > 1 {
                        ui.add_space(17.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new("DECISION QUESTION").size(11.0).strong().color(MUTED));
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            changed |= ui.add_sized([258.0, 58.0],
                                egui::TextEdit::multiline(&mut node.decision_question)
                                    .hint_text("What should Jev decide?"))
                                .changed();
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.add_sized([252.0, 32.0], egui::Label::new(
                                RichText::new(if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) {
                                    "If empty, Jev uses this block's name."
                                } else {
                                    "If empty, Jev uses a general routing question."
                                }).size(11.0).color(MUTED)).wrap());
                        });
                    }
                    if node.kind == NodeKind::Decision {
                        ui.add_space(18.0);
                        ui.label(RichText::new("PATHS").size(11.0).strong().color(MUTED));
                        for route in &mut node.routes {
                            changed |= ui.add_sized([250.0, 30.0], egui::TextEdit::singleline(route)).changed();
                        }
                        ui.horizontal(|ui| {
                            if node.routes.len() < 6 && ui.small_button("+ Add path").clicked() {
                                node.routes.push(format!("Path {}", node.routes.len() + 1));
                                changed = true;
                            }
                            if node.routes.len() > 1 && ui.small_button("Remove last").clicked() {
                                remove_last_route = true;
                            }
                        });
                    }
                    if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) || branch_targets.len() > 1 {
                        ui.add_space(16.0);
                        egui::CollapsingHeader::new("Routing settings").id_salt(("routing_settings", id)).show(ui, |ui| {
                            ui.label(RichText::new("Ask me when confidence is below").size(12.0).color(MUTED));
                            changed |= ui.add(egui::Slider::new(&mut node.threshold, 0.0..=1.0).show_value(true)).changed();
                        });
                    }
                }
                if remove_last_route {
                    if let Some(canvas) = self.canvas_mut()
                        && let Some(node) = canvas.node_mut(id) {
                            node.routes.pop();
                            node.mock_route =
                                node.mock_route.min(node.routes.len().saturating_sub(1));
                            let count = node.routes.len();
                            canvas
                                .edges
                                .retain(|edge| edge.from != id || edge.output < count);
                        }
                    changed = true;
                }
                if changed {
                    self.changed();
                }
                ui.add_space(26.0);
                ui.separator();
                ui.add_space(15.0);
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("CONNECTIONS")
                            .size(11.0)
                            .strong()
                            .color(MUTED),
                    );
                });
                ui.add_space(9.0);
                let edge_labels = self
                    .canvas()
                    .map(|c| {
                        c.edges
                            .iter()
                            .filter(|e| e.from == id || e.to == id)
                            .map(|e| {
                                let other = if e.from == id { e.to } else { e.from };
                                let name = c
                                    .node(other)
                                    .map(|n| n.title.clone())
                                    .unwrap_or_else(|| "Unknown".into());
                                (
                                    e.id,
                                    format!("{}  {}", if e.from == id { "To" } else { "From" }, name),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                if edge_labels.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new("No connections yet").size(12.0).color(MUTED));
                    });
                }
                for (edge_id, label) in edge_labels {
                    ui.horizontal(|ui| {
                        ui.add_space(13.0);
                        ui.label(RichText::new(label).size(12.0).color(TEXT));
                        if ui
                            .small_button(ICON_CLOSE)
                            .on_hover_text("Remove connection")
                            .clicked()
                            && let Some(canvas) = self.canvas_mut() {
                                canvas.edges.retain(|e| e.id != edge_id);
                                self.changed();
                            }
                    });
                }
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(13.0);
                    if ui.button("Duplicate").clicked() {
                        self.duplicate_selected();
                    }
                    if ui.button("Delete").clicked() {
                        self.delete_selected();
                    }
                });
                    });
            });
    }

    fn run_panel(&mut self, ctx: &egui::Context) {
        let height = if !self.show_run_log {
            32.0
        } else if self.run.send_approval {
            385.0
        } else if self.run.approval.is_some() {
            238.0
        } else if self.run.artifact.is_some() && !self.run.send_approval {
            235.0
        } else {
            180.0
        };
        egui::TopBottomPanel::bottom("run_log")
            .exact_height(height)
            .show(ctx, |ui| {
                if self.dialog_open() {
                    ui.disable();
                }
                ui.horizontal(|ui| {
                    ui.add_space(13.0);
                    if ui
                        .add(
                            egui::Button::new(if self.show_run_log {
                                ICON_EXPAND_MORE
                            } else {
                                ICON_EXPAND_LESS
                            })
                            .fill(PANEL)
                            .frame(false),
                        )
                        .clicked()
                    {
                        self.show_run_log = !self.show_run_log;
                    }
                    ui.label(RichText::new("ACTIVITY").size(11.0).strong().color(MUTED));
                    ui.add_space(9.0);
                    let status = if self.run.approval.is_some() {
                        "Needs approval"
                    } else if self.run.send_approval {
                        "Review reply"
                    } else if self.run.pending.is_some() || self.run.pending_action.is_some() {
                        "Waiting for service"
                    } else if self.run.mail_polling {
                        "Watching inbox"
                    } else if self.run.paused {
                        "Paused"
                    } else if self.run.running() {
                        "Running"
                    } else if self.run.finished {
                        "Finished"
                    } else {
                        "Ready"
                    };
                    ui.label(
                        RichText::new(status)
                            .size(11.0)
                            .color(if self.run.running() {
                                ACCENT_DETAIL
                            } else {
                                MUTED
                            }),
                    );
                });
                if self.show_run_log {
                    if !self.run.send_approval {
                        if let Some(file) = &self.run.artifact {
                            let title = file.title.clone();
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(RichText::new(title).color(ACCENT_DETAIL));
                                if ui.small_button("Open file").clicked() {
                                    self.open_generated_file();
                                }
                            });
                        }
                        if self.run.mode == RunMode::Jev && !self.run.draft.is_empty() {
                            egui::ScrollArea::vertical()
                                .id_salt("workflow_output")
                                .max_height(80.0)
                                .show(ui, |ui| {
                                    ui.label(&self.run.draft);
                                });
                        }
                    }
                    if self.run.send_approval {
                        let address = self
                            .run
                            .mail
                            .as_ref()
                            .map(|mail| mail.recipient.as_str())
                            .unwrap_or("Unknown sender");
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!("REPLY TO  {address}"))
                                    .size(11.0)
                                    .strong()
                                    .color(MUTED),
                            );
                        });
                        if let Some(file) = &self.run.artifact {
                            let filename = file.filename.clone();
                            let size = file.bytes.len();
                            ui.horizontal(|ui| {
                                ui.add_space(16.0);
                                ui.label(
                                    RichText::new(format!(
                                        "ATTACHMENT  {filename} · {} KB",
                                        size.div_ceil(1024)
                                    ))
                                    .size(11.0)
                                    .strong()
                                    .color(ACCENT_DETAIL),
                                );
                                if ui.small_button("Open file").clicked() {
                                    self.open_generated_file();
                                }
                            });
                        }
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.add_sized(
                                [ui.available_width() - 20.0, 172.0],
                                egui::TextEdit::multiline(&mut self.run.draft)
                                    .hint_text("Review and edit the reply"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            if ui.add(primary_button("Approve & send")).clicked() {
                                self.approve_send();
                            }
                            if ui.button("Cancel run").clicked() {
                                self.run.stop();
                            }
                        });
                    }
                    let mut approved = None;
                    if let Some(approval) = &self.run.approval {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(
                                RichText::new(format!(
                                    "Jev confidence {:.0}% is below {:.0}%. Choose a path:",
                                    approval.confidence * 100.0,
                                    approval.threshold * 100.0,
                                ))
                                .size(12.0)
                                .color(TEXT),
                            );
                        });
                        ui.horizontal_wrapped(|ui| {
                            ui.add_space(16.0);
                            for (index, label) in approval.labels.iter().enumerate() {
                                let title = if index == approval.suggested {
                                    format!("{}  {} · Jev's pick", ICON_CHECK, label)
                                } else {
                                    label.clone()
                                };
                                if ui.button(title).clicked() {
                                    approved = Some(index);
                                }
                            }
                        });
                    }
                    if let Some(index) = approved {
                        self.approve_run_edge(index);
                    }
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if self.run.log.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.add_space(17.0);
                                    ui.label(
                                        RichText::new("Press Play to run your connected blocks.")
                                            .size(12.0)
                                            .color(MUTED),
                                    );
                                });
                            }
                            for (index, event) in self.run.log.iter().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add_space(17.0);
                                    ui.label(
                                        RichText::new(format!("{:02}", index + 1))
                                            .monospace()
                                            .size(11.0)
                                            .color(MUTED),
                                    );
                                    ui.label(RichText::new(event).size(12.0).color(TEXT));
                                });
                            }
                        });
                }
            });
    }

    fn canvas_ui(&mut self, ctx: &egui::Context) {
        let dialog_open = self.dialog_open();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG))
            .show(ctx, |ui| {
                if dialog_open {
                    ui.disable();
                }
                let rect = ui.max_rect();
                if self.fit_canvas {
                    self.fit_canvas = false;
                    let mut bounds = Rect::NOTHING;
                    if let Some(canvas) = self.canvas() {
                        for node in &canvas.nodes {
                            bounds.extend_with(Pos2::new(node.x, node.y));
                            bounds.extend_with(Pos2::new(node.x + 248.0, node.y + 150.0));
                        }
                    }
                    if bounds.is_finite() {
                        self.pan = bounds.center().to_vec2();
                        self.zoom = ((rect.width() - 100.0) / bounds.width())
                            .min((rect.height() - 100.0) / bounds.height())
                            .clamp(0.2, 1.0);
                    }
                }
                let response = ui.interact(rect, Id::new("canvas_background"), Sense::click());
                ui.painter().rect_filled(rect, 0, BG);
                self.draw_grid(ui, rect);
                let pointer = if dialog_open {
                    None
                } else {
                    ui.input(|i| i.pointer.hover_pos())
                };
                let hit_node = pointer.is_some_and(|p| {
                    self.canvas().is_some_and(|c| {
                        c.nodes.iter().any(|n| self.node_rect(rect, n).contains(p))
                    })
                });
                let pressed = !dialog_open
                    && self.connection_menu.is_none()
                    && ui.input(|i| i.pointer.primary_pressed());
                let down = ui.input(|i| i.pointer.primary_down());
                let edge_hit = if pressed || response.clicked() {
                    pointer
                        .filter(|p| rect.contains(*p) && !hit_node)
                        .and_then(|p| self.hit_edge(rect, p))
                } else {
                    None
                };
                if pressed {
                    if let Some((from, output)) = pointer
                        .and_then(|p| self.hit_output(rect, p))
                        .filter(|_| !self.run.running())
                    {
                        self.connecting = Some((from, output));
                        self.connection_dragging = true;
                        self.connection_menu = None;
                        self.selected = Some(from);
                        self.selected_nodes.clear();
                        self.selected_nodes.insert(from);
                        self.selected_edge = None;
                    } else if let Some(edge_id) = edge_hit {
                        self.selected_edge = Some(edge_id);
                        self.selected = None;
                        self.selected_nodes.clear();
                        self.connecting = None;
                    }
                    if !self.connection_dragging && edge_hit.is_none() {
                        let hit = pointer.and_then(|p| {
                            self.canvas()?.nodes.iter().rev().find_map(|node| {
                                self.node_rect(rect, node)
                                    .shrink2(Vec2::new(17.0 * self.zoom, 6.0 * self.zoom))
                                    .contains(p)
                                    .then_some(node.id)
                            })
                        });
                        let multi = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                        if let (Some(id), Some(p)) = (hit, pointer) {
                            if multi {
                                if !self.selected_nodes.insert(id) {
                                    self.selected_nodes.remove(&id);
                                }
                            } else if !self.selected_nodes.contains(&id) {
                                self.selected_nodes.clear();
                                self.selected_nodes.insert(id);
                            }
                            self.selected = self
                                .selected_nodes
                                .contains(&id)
                                .then_some(id)
                                .or_else(|| self.selected_nodes.iter().next().copied());
                            self.selected_edge = None;
                            self.connecting = None;
                            let positions = self
                                .canvas()
                                .map(|canvas| {
                                    canvas
                                        .nodes
                                        .iter()
                                        .filter(|node| self.selected_nodes.contains(&node.id))
                                        .map(|node| (node.id, Vec2::new(node.x, node.y)))
                                        .collect::<Vec<_>>()
                                })
                                .unwrap_or_default();
                            self.node_drag =
                                (!self.run.running() && !multi && !positions.is_empty())
                                    .then_some((p, positions));
                            self.marquee = None;
                        } else if !hit_node
                            && edge_hit.is_none()
                            && !ui.input(|i| i.key_down(egui::Key::Space))
                            && let Some(p) = pointer.filter(|p| rect.contains(*p))
                        {
                            let initial = if multi {
                                self.selected_nodes.clone()
                            } else {
                                HashSet::new()
                            };
                            self.marquee = Some((p, p, initial));
                            self.node_drag = None;
                        }
                    }
                }
                if let (Some((origin, positions)), Some(p)) = (&self.node_drag, pointer) {
                    let offset = (p - *origin) / self.zoom;
                    if offset.length_sq() > 4.0 {
                        let positions = positions.clone();
                        if let Some(canvas) = self.canvas_mut() {
                            for (id, initial) in positions {
                                if let Some(node) = canvas.node_mut(id) {
                                    node.x = initial.x + offset.x;
                                    node.y = initial.y + offset.y;
                                }
                            }
                            self.selected_edge = None;
                            self.changed();
                        }
                    }
                }
                if let (Some((start, _, initial)), Some(p)) = (&self.marquee, pointer)
                    && down
                {
                    let area = Rect::from_two_pos(*start, p);
                    let mut selection = initial.clone();
                    if let Some(canvas) = self.canvas() {
                        for node in &canvas.nodes {
                            if area.intersects(self.node_rect(rect, node)) {
                                selection.insert(node.id);
                            }
                        }
                    }
                    self.selected_nodes = selection;
                    self.selected = self.selected_nodes.iter().next().copied();
                    self.selected_edge = None;
                    if let Some(marquee) = &mut self.marquee {
                        marquee.1 = p;
                    }
                }
                if !down {
                    self.node_drag = None;
                    self.marquee = None;
                }
                if pressed {
                    self.pan_dragging = response.hovered()
                        && !hit_node
                        && ui.input(|i| i.key_down(egui::Key::Space));
                }
                if !down {
                    self.pan_dragging = false;
                }
                if self.pan_dragging && down && self.connecting.is_none() {
                    self.pan -= ui.input(|i| i.pointer.delta()) / self.zoom;
                }
                if response.clicked()
                    && !hit_node
                    && edge_hit.is_none()
                    && self.connection_menu.is_none()
                {
                    self.selected = None;
                    self.selected_nodes.clear();
                    self.selected_edge = None;
                    self.connecting = None;
                    self.connection_menu = None;
                }
                let pointer_over_block_menu = pointer.is_some_and(|p| {
                    self.block_menu_rect
                        .is_some_and(|menu_rect| menu_rect.contains(p))
                });
                if pointer.is_some_and(|p| rect.contains(p)) && !pointer_over_block_menu {
                    // Follow the trackpad immediately; macOS still sends momentum on release.
                    let (scroll, pinch, command) =
                        ui.input(|i| (i.raw_scroll_delta, i.zoom_delta(), i.modifiers.command));
                    if (pinch - 1.0).abs() > 0.001 || (command && scroll.y.abs() > 0.1) {
                        let pointer = pointer.unwrap_or(rect.center());
                        let before = self.screen_to_world(rect, pointer);
                        let factor = if (pinch - 1.0).abs() > 0.001 {
                            pinch
                        } else {
                            (scroll.y * 0.002).exp()
                        };
                        self.zoom = (self.zoom * factor).clamp(0.2, 2.5);
                        let after = self.screen_to_world(rect, pointer);
                        self.pan += before - after;
                    } else if !command && scroll.length_sq() > 0.01 {
                        self.pan -= scroll / self.zoom;
                    }
                }
                if response.secondary_clicked() {
                    let at = response.interact_pointer_pos().unwrap_or(rect.center());
                    let offset = at - rect.center();
                    self.context_menu_canvas_pos = Some(Pos2::new(offset.x, offset.y));
                    self.menu_picker = BlockPickerState::default();
                }
                egui::Popup::context_menu(&response)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| {
                        if self.run.running() {
                            ui.disable();
                        }
                        ui.label(RichText::new("ADD BLOCK").size(10.0).strong().color(MUTED));
                        ui.add_space(4.0);
                        if let Some(kind) =
                            block_picker(ui, &mut self.menu_picker, 224.0, false, 300.0)
                        {
                            let canvas_pos = self.context_menu_canvas_pos.unwrap_or(Pos2::ZERO);
                            self.add_node(kind, Some(canvas_pos));
                            self.menu_picker = BlockPickerState::default();
                            self.context_menu_canvas_pos = None;
                            ui.close();
                        }
                    });
                self.draw_edges(ui, rect);
                self.draw_nodes(ui, rect);
                if let Some((start, end, _)) = &self.marquee {
                    let area = Rect::from_two_pos(*start, *end);
                    ui.painter()
                        .rect_filled(area, 0, ACCENT.gamma_multiply(0.12));
                    ui.painter().rect_stroke(
                        area,
                        0,
                        Stroke::new(1.0_f32, ACCENT_DETAIL),
                        StrokeKind::Inside,
                    );
                }
                if self.connection_dragging && !down {
                    if let Some(p) = pointer {
                        if let Some(to) = self.hit_input(rect, p) {
                            self.connect_to(to);
                        } else if rect.contains(p) && !self.hit_any_node(rect, p) {
                            let menu_at = Pos2::new(
                                p.x.min(rect.right() - 260.0).max(rect.left() + 8.0),
                                p.y.min(rect.bottom() - 370.0).max(rect.top() + 8.0),
                            );
                            let canvas_pos = p - rect.center();
                            if let Some(source) = self.connecting.take() {
                                self.connection_menu =
                                    Some((menu_at, Pos2::new(canvas_pos.x, canvas_pos.y), source));
                                self.menu_picker = BlockPickerState::with_search_focused();
                            }
                        }
                    }
                    self.connection_dragging = false;
                }
                if let Some((from, output)) = self.connecting
                    && let Some(node) = self.canvas().and_then(|c| c.node(from))
                {
                    let start = self.output_pos(rect, node, output);
                    let end = ui.input(|i| i.pointer.hover_pos()).unwrap_or(start);
                    paint_wire(ui.painter(), start, end, ACCENT_DETAIL, 2.0, false);
                }
                if self.canvas().is_some_and(|c| c.nodes.is_empty()) {
                    let welcome = Rect::from_center_size(rect.center(), Vec2::new(390.0, 190.0));
                    ui.scope_builder(egui::UiBuilder::new().max_rect(welcome), |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new("Start with a trigger").size(23.0).strong());
                            ui.label(
                                RichText::new("Then drag a connection to add the next step.")
                                    .size(13.0)
                                    .color(MUTED),
                            );
                            ui.add_space(20.0);
                            let mut chosen = None;
                            ui.horizontal(|ui| {
                                ui.add_space(39.0);
                                if ui
                                    .add_sized(
                                        [145.0, 40.0],
                                        egui::Button::new(format!("{}  Email received", ICON_MAIL))
                                            .corner_radius(8),
                                    )
                                    .clicked()
                                {
                                    chosen = Some(NodeKind::GmailInbox);
                                }
                                if ui
                                    .add_sized(
                                        [135.0, 40.0],
                                        egui::Button::new(format!("{}  Play", ICON_PLAY_ARROW))
                                            .corner_radius(8),
                                    )
                                    .clicked()
                                {
                                    chosen = Some(NodeKind::Start);
                                }
                            });
                            if let Some(kind) = chosen {
                                let pan = self.pan;
                                if let Some(id) = self.add_node(kind, None)
                                    && let Some(node) =
                                        self.canvas_mut().and_then(|c| c.node_mut(id))
                                {
                                    node.x = pan.x - 124.0;
                                    node.y = pan.y - 45.0;
                                }
                            }
                        });
                    });
                }
                let zoom_label = format!("{}%", (self.zoom * 100.0).round() as i32);
                let controls = Rect::from_min_size(
                    Pos2::new(rect.right() - 111.0, rect.bottom() - 52.0),
                    Vec2::new(96.0, 36.0),
                );
                ui.painter().rect_filled(controls, 8, CARD);
                ui.painter().rect_stroke(
                    controls,
                    8,
                    Stroke::new(1.0_f32, LINE),
                    StrokeKind::Inside,
                );
                let zoom_response = ui
                    .interact(controls, Id::new("zoom_reset"), Sense::click())
                    .on_hover_text("Fit workflow to view");
                ui.painter().text(
                    controls.center(),
                    egui::Align2::CENTER_CENTER,
                    zoom_label,
                    FontId::proportional(12.0),
                    TEXT,
                );
                if zoom_response.clicked() {
                    self.fit_canvas = true;
                }
            });
    }

    fn screen_to_world(&self, rect: Rect, point: Pos2) -> Vec2 {
        (point - rect.center()) / self.zoom + self.pan
    }
    fn world_to_screen(&self, rect: Rect, point: Vec2) -> Pos2 {
        rect.center() + (point - self.pan) * self.zoom
    }

    fn hit_any_node(&self, rect: Rect, point: Pos2) -> bool {
        self.canvas().is_some_and(|canvas| {
            canvas
                .nodes
                .iter()
                .any(|node| self.node_rect(rect, node).contains(point))
        })
    }

    fn hit_output(&self, rect: Rect, point: Pos2) -> Option<(u64, usize)> {
        self.canvas()?.nodes.iter().rev().find_map(|node| {
            (0..node.output_count()).find_map(|output| {
                (self.output_pos(rect, node, output).distance(point) <= 12.0 * self.zoom.max(0.7))
                    .then_some((node.id, output))
            })
        })
    }

    fn hit_input(&self, rect: Rect, point: Pos2) -> Option<u64> {
        self.canvas()?
            .nodes
            .iter()
            .rev()
            .find(|node| {
                node.kind.accepts_input()
                    && (self.node_rect(rect, node).contains(point)
                        || Pos2::new(
                            self.node_rect(rect, node).left(),
                            self.node_rect(rect, node).center().y,
                        )
                        .distance(point)
                            <= 14.0 * self.zoom.max(0.7))
            })
            .map(|node| node.id)
    }

    fn draw_grid(&self, ui: &egui::Ui, rect: Rect) {
        let step = 30.0 * self.zoom;
        if step < 14.0 {
            return;
        }
        let mut dots = egui::Mesh::default();
        let dot_color = Color32::from_rgb(50, 54, 55);
        let center = self.world_to_screen(rect, Vec2::ZERO);
        let mut x = rect.left() + (center.x - rect.left()).rem_euclid(step);
        while x < rect.right() {
            let mut y = rect.top() + (center.y - rect.top()).rem_euclid(step);
            while y < rect.bottom() {
                dots.add_colored_rect(
                    Rect::from_center_size(Pos2::new(x, y), Vec2::splat(1.5)),
                    dot_color,
                );
                y += step;
            }
            x += step;
        }
        ui.painter().add(egui::Shape::mesh(dots));
    }

    fn node_rect(&self, canvas_rect: Rect, node: &Node) -> Rect {
        let pos = self.world_to_screen(canvas_rect, Vec2::new(node.x, node.y));
        let height = if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) {
            70.0 + node.output_count() as f32 * 18.0
        } else {
            82.0
        };
        Rect::from_min_size(pos, Vec2::new(250.0, height) * self.zoom)
    }

    fn output_pos(&self, canvas_rect: Rect, node: &Node, output: usize) -> Pos2 {
        let rect = self.node_rect(canvas_rect, node);
        if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) {
            Pos2::new(
                rect.right(),
                rect.top() + (69.0 + output as f32 * 18.0) * self.zoom,
            )
        } else {
            Pos2::new(rect.right(), rect.center().y)
        }
    }

    fn draw_edges(&self, ui: &egui::Ui, rect: Rect) {
        let Some(canvas) = self.canvas() else {
            return;
        };
        for edge in &canvas.edges {
            let (Some(from), Some(to)) = (canvas.node(edge.from), canvas.node(edge.to)) else {
                continue;
            };
            let start = self.output_pos(rect, from, edge.output);
            let end = Pos2::new(
                self.node_rect(rect, to).left(),
                self.node_rect(rect, to).center().y,
            );
            if !Rect::from_two_pos(start, end)
                .expand(130.0)
                .intersects(rect)
            {
                continue;
            }
            let active = self.run.traversed_edges.contains(&edge.id);
            let selected = self.selected_edge == Some(edge.id);
            paint_wire(
                ui.painter(),
                start,
                end,
                if active || selected {
                    ACCENT_DETAIL
                } else {
                    Color32::from_rgb(117, 124, 124)
                },
                if active || selected { 2.8 } else { 1.7 },
                active,
            );
        }
    }

    fn hit_edge(&self, rect: Rect, pointer: Pos2) -> Option<u64> {
        let canvas = self.canvas()?;
        canvas.edges.iter().rev().find_map(|edge| {
            let from = canvas.node(edge.from)?;
            let to = canvas.node(edge.to)?;
            let start = self.output_pos(rect, from, edge.output);
            let to_rect = self.node_rect(rect, to);
            let end = Pos2::new(to_rect.left(), to_rect.center().y);
            if !Rect::from_two_pos(start, end)
                .expand(125.0)
                .contains(pointer)
            {
                return None;
            }
            let mut previous = start;
            for step in 1..=32 {
                let next = wire_point(start, end, step as f32 / 32.0);
                if distance_to_segment(pointer, previous, next) <= 7.0 {
                    return Some(edge.id);
                }
                previous = next;
            }
            None
        })
    }

    fn draw_nodes(&mut self, ui: &mut egui::Ui, canvas_rect: Rect) {
        let nodes = self
            .canvas()
            .map(|c| {
                c.nodes
                    .iter()
                    .filter(|node| self.node_rect(canvas_rect, node).intersects(canvas_rect))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let pointer = ui.input(|i| i.pointer.hover_pos());
        for node in nodes {
            let rect = self.node_rect(canvas_rect, &node);
            let selected = self.selected_nodes.contains(&node.id);
            let active = self.run.active == Some(node.id) && self.run.running();
            let visited = self.run.visited.contains(&node.id);
            ui.painter().rect_filled(rect, CornerRadius::same(12), CARD);
            let border = if active {
                ACCENT_DETAIL
            } else if selected {
                Color32::from_rgb(193, 217, 204)
            } else if visited {
                Color32::from_rgb(91, 122, 135)
            } else {
                LINE
            };
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(12),
                Stroke::new(if selected || active { 1.8_f32 } else { 1.0_f32 }, border),
                StrokeKind::Inside,
            );
            let icon = Rect::from_min_size(
                rect.min + Vec2::new(15.0, 17.0) * self.zoom,
                Vec2::splat(38.0 * self.zoom),
            );
            let color = kind_color(node.kind);
            ui.painter()
                .rect_filled(icon, CornerRadius::same(9), color.gamma_multiply(0.16));
            ui.painter().text(
                icon.center(),
                egui::Align2::CENTER_CENTER,
                kind_icon(node.kind),
                FontId::proportional(20.0 * self.zoom),
                color,
            );
            let title_x = rect.left() + 65.0 * self.zoom;
            let title_y = rect.top() + 25.0 * self.zoom;
            let title = if node.title.chars().count() > 22 {
                format!("{}…", node.title.chars().take(21).collect::<String>())
            } else {
                node.title.clone()
            };
            ui.painter().text(
                Pos2::new(title_x, title_y),
                egui::Align2::LEFT_CENTER,
                title,
                FontId::proportional(14.0 * self.zoom),
                TEXT,
            );
            let subtitle = if node.subtitle.chars().count() > 30 {
                format!("{}…", node.subtitle.chars().take(29).collect::<String>())
            } else {
                node.subtitle.clone()
            };
            ui.painter().text(
                Pos2::new(title_x, title_y + 22.0 * self.zoom),
                egui::Align2::LEFT_CENTER,
                subtitle,
                FontId::proportional(10.5 * self.zoom),
                MUTED,
            );
            if matches!(node.kind, NodeKind::Decision | NodeKind::YesNo) {
                for index in 0..node.output_count() {
                    let port = self.output_pos(canvas_rect, &node, index);
                    ui.painter().text(
                        port - Vec2::new(13.0 * self.zoom, 0.0),
                        egui::Align2::RIGHT_CENTER,
                        node.output_label(index),
                        FontId::proportional(10.0 * self.zoom),
                        MUTED,
                    );
                }
            }
            if node.kind.accepts_input() {
                let port = Pos2::new(rect.left(), rect.center().y);
                ui.painter().circle_filled(
                    port,
                    5.0 * self.zoom,
                    if self.connecting.is_some() {
                        ACCENT_DETAIL
                    } else {
                        PANEL
                    },
                );
                ui.painter().circle_stroke(
                    port,
                    5.0 * self.zoom,
                    Stroke::new(
                        1.5_f32,
                        if self.connecting.is_some() {
                            ACCENT_DETAIL
                        } else {
                            MUTED
                        },
                    ),
                );
                let port_rect =
                    Rect::from_center_size(port, Vec2::splat(24.0 * self.zoom.max(0.7)));
                if ui
                    .interact(port_rect, Id::new(("in", node.id)), Sense::click())
                    .clicked()
                {
                    self.connect_to(node.id);
                }
            }
            for output in 0..node.output_count() {
                let port = self.output_pos(canvas_rect, &node, output);
                ui.painter().circle_filled(port, 5.0 * self.zoom, PANEL);
                ui.painter().circle_stroke(
                    port,
                    5.0 * self.zoom,
                    Stroke::new(
                        1.5_f32,
                        if self.connecting == Some((node.id, output)) {
                            ACCENT_DETAIL
                        } else {
                            MUTED
                        },
                    ),
                );
                let port_rect =
                    Rect::from_center_size(port, Vec2::splat(24.0 * self.zoom.max(0.7)));
                if ui
                    .interact(port_rect, Id::new(("out", node.id, output)), Sense::click())
                    .clicked()
                    && !self.run.running()
                {
                    self.connecting = Some((node.id, output));
                    self.selected = Some(node.id);
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(node.id);
                    self.selected_edge = None;
                }
            }
            let body = rect.shrink2(Vec2::new(17.0 * self.zoom, 6.0 * self.zoom));
            let response = ui.interact(body, Id::new(("node", node.id)), Sense::click_and_drag());
            if response.clicked()
                && !ui.input(|i| i.modifiers.ctrl || i.modifiers.command)
                && self.selected_nodes.len() <= 1
            {
                self.selected = Some(node.id);
                self.selected_nodes.clear();
                self.selected_nodes.insert(node.id);
                self.selected_edge = None;
                self.connecting = None;
            }
            response.context_menu(|ui| {
                if self.run.running() {
                    ui.disable();
                }
                if ui.button("Duplicate").clicked() {
                    self.selected = Some(node.id);
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(node.id);
                    self.duplicate_selected();
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    self.selected = Some(node.id);
                    self.selected_nodes.clear();
                    self.selected_nodes.insert(node.id);
                    self.delete_selected();
                    ui.close();
                }
            });
        }
        if self.connecting.is_some() && pointer.is_some_and(|p| !canvas_rect.contains(p)) {
            self.connecting = None;
        }
    }

    fn integration_details(&mut self, ui: &mut egui::Ui) {
        ui.set_min_height(300.0);
        match self.integration_tab {
            0 => {
                ui.heading("AI model");
                ui.label(RichText::new("Writes replies and creates your documents.").color(MUTED));
                ui.label(
                    RichText::new("Use your own API key. Your provider's usage charges apply.")
                        .size(11.0)
                        .color(MUTED),
                );
                let keys_url = match self.llm_provider {
                    llm::Provider::OpenRouter => "https://openrouter.ai/settings/keys",
                    llm::Provider::OpenAI => "https://platform.openai.com/api-keys",
                    llm::Provider::Anthropic => "https://console.anthropic.com/",
                };
                ui.hyperlink_to(
                    format!("Get an {} API key", self.llm_provider.label()),
                    keys_url,
                );
                ui.add_space(15.0);
                egui::ComboBox::from_id_salt("llm_provider")
                    .width(400.0)
                    .selected_text(self.llm_provider.label())
                    .show_ui(ui, |ui| {
                        for provider in llm::Provider::ALL {
                            if ui
                                .selectable_value(
                                    &mut self.llm_provider,
                                    provider,
                                    provider.label(),
                                )
                                .clicked()
                            {
                                self.llm_model = provider.default_model().into();
                                self.llm_key_draft.zeroize();
                                self.llm_key_present = Some(
                                    self.provider_keys[llm::Provider::ALL
                                        .iter()
                                        .position(|p| *p == provider)
                                        .unwrap_or(0)],
                                );
                            }
                        }
                    });
                ui.label("Model");
                ui.add_sized(
                    [410.0, 34.0],
                    egui::TextEdit::singleline(&mut self.llm_model),
                );
                ui.label("API key");
                ui.add_sized(
                    [410.0, 34.0],
                    egui::TextEdit::singleline(&mut *self.llm_key_draft)
                        .password(true)
                        .hint_text(if self.llm_key_present == Some(true) {
                            "Key saved securely"
                        } else {
                            "Paste your provider API key"
                        }),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save connection").clicked() {
                        let result = (|| -> Result<(), String> {
                            if self.llm_model.trim().is_empty() {
                                return Err("Enter a model ID".into());
                            }
                            if self.llm_key_draft.trim().is_empty()
                                && self.llm_key_present != Some(true)
                            {
                                return Err("Enter your provider API key".into());
                            }
                            credentials::save_named("llm-provider", self.llm_provider.label())?;
                            credentials::save_named("llm-model", &self.llm_model)?;
                            if !self.llm_key_draft.trim().is_empty() {
                                credentials::save_named(
                                    self.llm_provider.key_name(),
                                    &self.llm_key_draft,
                                )?;
                                self.llm_key_draft.zeroize();
                                self.llm_key_present = Some(true);
                                self.provider_keys[llm::Provider::ALL
                                    .iter()
                                    .position(|p| *p == self.llm_provider)
                                    .unwrap_or(0)] = true;
                            }
                            Ok(())
                        })();
                        match result {
                            Ok(()) => self.message("AI connection saved"),
                            Err(e) => self.message(e),
                        }
                    }
                    if self.llm_key_present == Some(true) && ui.button("Remove key").clicked() {
                        match credentials::remove_named(self.llm_provider.key_name()) {
                            Ok(()) => {
                                self.llm_key_present = Some(false);
                                self.provider_keys[llm::Provider::ALL
                                    .iter()
                                    .position(|p| *p == self.llm_provider)
                                    .unwrap_or(0)] = false;
                                self.message("AI key removed");
                            }
                            Err(e) => self.message(e),
                        }
                    }
                });
            }
            1 => {
                ui.heading("Jev decisions");
                ui.label(
                    RichText::new("Chooses a path whenever a block has several connections.")
                        .color(MUTED),
                );
                ui.add_space(15.0);
                ui.hyperlink_to("Get a TypeSafe API key", "https://console.typesafe.ai/");
                ui.label("API key");
                ui.add_sized(
                    [410.0, 34.0],
                    egui::TextEdit::singleline(&mut *self.key_draft)
                        .password(true)
                        .hint_text(if self.key_present == Some(true) {
                            "Key saved securely"
                        } else {
                            "Paste your TypeSafe API key"
                        }),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save connection").clicked() {
                        match credentials::save(&self.key_draft) {
                            Ok(()) => {
                                self.key_draft.zeroize();
                                self.key_present = Some(true);
                                self.message("Jev connected");
                            }
                            Err(e) => self.message(e),
                        }
                    }
                    if self.key_present == Some(true) && ui.button("Remove key").clicked() {
                        match credentials::remove() {
                            Ok(()) => {
                                self.key_present = Some(false);
                                self.message("Jev key removed");
                            }
                            Err(e) => self.message(e),
                        }
                    }
                });
                ui.add_space(20.0);
                ui.label(RichText::new("Uncertain decisions pause for your input. Change the confidence level in a block's routing settings.").color(MUTED));
            }
            2 => {
                ui.heading("Web research");
                ui.label(
                    RichText::new("Tavily finds current sources for research blocks.").color(MUTED),
                );
                ui.add_space(15.0);
                ui.hyperlink_to("Get a Tavily API key", "https://app.tavily.com/");
                ui.label("API key");
                ui.add_sized(
                    [410.0, 34.0],
                    egui::TextEdit::singleline(&mut *self.search_key_draft)
                        .password(true)
                        .hint_text(if self.search_key_present == Some(true) {
                            "Key saved securely"
                        } else {
                            "Paste your Tavily API key"
                        }),
                );
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button("Save connection").clicked() {
                        match search::save_key(&self.search_key_draft) {
                            Ok(()) => {
                                self.search_key_draft.zeroize();
                                self.search_key_present = Some(true);
                                self.message("Web research connected");
                            }
                            Err(e) => self.message(e),
                        }
                    }
                    if self.search_key_present == Some(true) && ui.button("Remove key").clicked() {
                        match search::remove_key() {
                            Ok(()) => {
                                self.search_key_present = Some(false);
                                self.message("Search key removed");
                            }
                            Err(e) => self.message(e),
                        }
                    }
                });
            }
            _ => {
                ui.heading("Gmail");
                ui.label(
                    RichText::new("Receive assignments and send replies after your approval.")
                        .color(MUTED),
                );
                ui.add_space(15.0);
                egui::CollapsingHeader::new("Google OAuth setup").default_open(self.gmail_client_id_draft.is_empty()).show(ui, |ui| {
                    ui.label(RichText::new("Use a Google Cloud Desktop OAuth client with Gmail read and send access.").size(12.0).color(MUTED));
                    ui.label("Client ID");
                    ui.add_sized([400.0, 34.0], egui::TextEdit::singleline(&mut self.gmail_client_id_draft));
                    ui.label("Client secret");
                    ui.add_sized([400.0, 34.0], egui::TextEdit::singleline(&mut *self.gmail_client_secret_draft).password(true).hint_text("Stored securely"));
                    if ui.button("Save OAuth client").clicked() {
                        match gmail::save_client(&self.gmail_client_id_draft, &self.gmail_client_secret_draft) {
                            Ok(()) => { self.gmail_client_secret_draft.zeroize(); self.message("Google OAuth client saved"); }
                            Err(e) => self.message(e),
                        }
                    }
                });
                ui.add_space(15.0);
                if self.gmail_connect_receiver.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Waiting for Google…");
                    });
                } else if self.gmail_connected == Some(true) {
                    ui.label(RichText::new("Gmail is connected").color(ACCENT_DETAIL));
                    if ui.button("Disconnect").clicked() {
                        match gmail::disconnect() {
                            Ok(()) => {
                                self.gmail_connected = Some(false);
                                self.message("Gmail disconnected");
                            }
                            Err(e) => self.message(e),
                        }
                    }
                } else if ui.button("Connect Gmail").clicked() {
                    let (sender, receiver) = mpsc::channel();
                    thread::spawn(move || {
                        let _ = sender.send(gmail::connect());
                    });
                    self.gmail_connect_receiver = Some(receiver);
                }
                ui.add_space(20.0);
                ui.label(RichText::new("Previous context is remembered automatically within each conversation. Email content goes to the providers used by your workflow.").size(12.0).color(MUTED));
            }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if let Some(receiver) = &self.gmail_connect_receiver {
            match receiver.try_recv() {
                Ok(Ok(())) => {
                    self.gmail_connect_receiver = None;
                    self.gmail_connected = Some(true);
                    self.message("Gmail connected");
                }
                Ok(Err(error)) => {
                    self.gmail_connect_receiver = None;
                    self.message(error);
                }
                Err(TryRecvError::Disconnected) => {
                    self.gmail_connect_receiver = None;
                    self.message("Gmail sign-in ended unexpectedly");
                }
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(150)),
            }
        }
        if self.show_integrations {
            if !self.integrations_loaded {
                self.load_connection_preferences();
            }
            let mut open = true;
            egui::Window::new("Integrations")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .fixed_size([640.0, 405.0])
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.add_space(8.0);
                    ui.allocate_ui_with_layout(
                        Vec2::new(620.0, 340.0),
                        egui::Layout::left_to_right(Align::Min),
                        |ui| {
                            ui.vertical(|ui| {
                                ui.set_width(145.0);
                                for (index, (icon, name, connected)) in [
                                    (
                                        ICON_PSYCHOLOGY,
                                        "AI model",
                                        self.llm_key_present == Some(true),
                                    ),
                                    (
                                        ICON_ALT_ROUTE,
                                        "Jev decisions",
                                        self.key_present == Some(true),
                                    ),
                                    (
                                        ICON_TRAVEL_EXPLORE,
                                        "Web research",
                                        self.search_key_present == Some(true),
                                    ),
                                    (ICON_MAIL, "Gmail", self.gmail_connected == Some(true)),
                                ]
                                .into_iter()
                                .enumerate()
                                {
                                    if ui
                                        .add_sized(
                                            [140.0, 40.0],
                                            egui::Button::new(format!("{icon}  {name}"))
                                                .selected(self.integration_tab == index),
                                        )
                                        .clicked()
                                    {
                                        self.integration_tab = index;
                                    }
                                    if connected {
                                        ui.label(
                                            RichText::new("     Connected")
                                                .size(10.0)
                                                .color(ACCENT_DETAIL),
                                        );
                                    }
                                }
                            });
                            ui.separator();
                            ui.vertical(|ui| {
                                ui.set_width(435.0);
                                egui::ScrollArea::vertical()
                                    .max_height(340.0)
                                    .show(ui, |ui| {
                                        if self.connection_receiver.is_some() {
                                            ui.set_min_height(300.0);
                                            ui.horizontal(|ui| {
                                                ui.spinner();
                                                ui.label("Loading saved connections…");
                                            });
                                        } else {
                                            self.integration_details(ui);
                                        }
                                    });
                            });
                        },
                    );
                    ui.add_space(12.0);
                    ui.separator();
                    ui.label(
                        RichText::new(
                            "Saved connections are encrypted and unlocked for this session.",
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                });
            self.show_integrations &= open;
            if !self.show_integrations {
                self.integrations_loaded = false;
                self.key_draft.zeroize();
                self.llm_key_draft.zeroize();
                self.search_key_draft.zeroize();
                self.gmail_client_secret_draft.zeroize();
            }
        }
        if let Some((menu_at, canvas_pos, source)) = self.connection_menu {
            let mut chosen = None;
            let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
            let menu_response = egui::Area::new(Id::new("connection_create_menu"))
                .order(egui::Order::Foreground)
                .fixed_pos(menu_at)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(CARD)
                        .corner_radius(10)
                        .stroke(Stroke::new(1.0_f32, LINE))
                        .inner_margin(9.0)
                        .show(ui, |ui| {
                            ui.set_min_width(224.0);
                            ui.label(
                                RichText::new("CREATE & CONNECT")
                                    .size(10.0)
                                    .strong()
                                    .color(MUTED),
                            );
                            ui.add_space(5.0);
                            chosen = block_picker(ui, &mut self.menu_picker, 224.0, true, 300.0);
                            ui.separator();
                            if ui.small_button("Cancel").clicked() {
                                cancel = true;
                            }
                        });
                });
            self.block_menu_rect = Some(menu_response.response.rect);
            let clicked_outside = ctx.input(|i| {
                i.pointer.primary_pressed()
                    && i.pointer
                        .press_origin()
                        .is_some_and(|p| !menu_response.response.rect.contains(p))
            });
            cancel |= clicked_outside;
            if let Some(kind) = chosen {
                if let Some(id) = self.add_node(kind, Some(canvas_pos)) {
                    self.connecting = Some(source);
                    self.connect_to(id);
                }
                self.connection_menu = None;
                self.menu_picker = BlockPickerState::default();
            } else if cancel {
                self.connection_menu = None;
                self.connecting = None;
                self.menu_picker = BlockPickerState::default();
            }
        } else {
            self.block_menu_rect = None;
        }
        if let Some((id, mut name)) = self.renaming_project.clone() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Rename project")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    let input = ui.add_sized(
                        [330.0, 36.0],
                        egui::TextEdit::singleline(&mut name).hint_text("Project name"),
                    );
                    if !ui.memory(|m| m.has_focus(input.id))
                        && !ui.memory(|m| m.focused().is_some())
                    {
                        input.request_focus();
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                        save = ui
                            .add_enabled(!name.trim().is_empty(), primary_button("Save name"))
                            .clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Enter));
                    });
                });
            if save && !name.trim().is_empty() {
                let mut project = self
                    .project
                    .as_ref()
                    .filter(|p| p.id == id)
                    .cloned()
                    .or_else(|| self.projects.iter().find(|p| p.id == id).cloned());
                if let Some(ref mut project) = project {
                    project.name = name.trim().to_owned();
                    match self.storage.save(project) {
                        Ok(()) => {
                            if self.project.as_ref().is_some_and(|p| p.id == id) {
                                self.project = Some(project.clone());
                            }
                            self.projects = self.storage.list();
                            self.message("Project renamed");
                            open = false;
                        }
                        Err(e) => self.message(format!("Could not rename project: {e}")),
                    }
                }
            }
            self.renaming_project = (open && !cancel).then_some((id, name));
        }
        if self.show_new_project {
            let mut open = true;
            egui::Window::new("New project")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Give this workspace a name").color(MUTED));
                    let response = ui.add_sized(
                        [310.0, 34.0],
                        egui::TextEdit::singleline(&mut self.new_project_name)
                            .hint_text("Project name"),
                    );
                    if self.focus_project_name {
                        response.request_focus();
                        self.focus_project_name = false;
                    }
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(10.0);
                    ui.label(RichText::new("Start with").size(12.0).color(MUTED));
                    ui.radio_value(&mut self.new_project_template, false, "Blank canvas");
                    ui.radio_value(
                        &mut self.new_project_template,
                        true,
                        "Text workflow · Play / AI model / End",
                    );
                    ui.label(
                        RichText::new(
                            "You can change every block later. AI steps use your own API key.",
                        )
                        .size(11.0)
                        .color(MUTED),
                    );
                    ui.add_space(8.0);
                    if ui
                        .add_enabled(
                            !self.new_project_name.trim().is_empty(),
                            primary_button("Create project"),
                        )
                        .clicked()
                        || enter
                    {
                        self.create_project();
                    }
                });
            self.show_new_project &= open;
            if !self.show_new_project {
                self.focus_project_name = true;
            }
        }
        if self.show_new_canvas {
            let mut open = true;
            egui::Window::new("New canvas")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("Name your canvas").color(MUTED));
                    let response = ui.add_sized(
                        [310.0, 34.0],
                        egui::TextEdit::singleline(&mut self.new_canvas_name)
                            .hint_text("e.g. Incoming requests"),
                    );
                    if self.focus_canvas_name {
                        response.request_focus();
                        self.focus_canvas_name = false;
                    }
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(10.0);
                    if ui
                        .add_enabled(
                            !self.new_canvas_name.trim().is_empty(),
                            primary_button("Create canvas"),
                        )
                        .clicked()
                        || enter
                    {
                        let name = self.new_canvas_name.trim().to_owned();
                        if !name.is_empty() {
                            if let Some(project) = self.project.as_mut() {
                                let id = project.allocate_id();
                                project.canvases.push(Canvas::new(id, name));
                                self.canvas_index = project.canvases.len() - 1;
                                self.selected = None;
                                self.selected_nodes.clear();
                                self.pan = Vec2::ZERO;
                                self.zoom = 1.0;
                                self.changed();
                            }
                            self.new_canvas_name.clear();
                            self.show_new_canvas = false;
                        }
                    }
                });
            self.show_new_canvas &= open;
            if !self.show_new_canvas {
                self.focus_canvas_name = true;
            }
        }
        if let Some(id) = self.deleting_project {
            let mut open = true;
            let name = self
                .projects
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.name.as_str())
                .unwrap_or("this project")
                .to_owned();
            egui::Window::new("Delete project?")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(340.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(format!("Delete “{name}” and its saved canvases?"));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            self.deleting_project = None;
                        }
                        if ui.button("Delete project").clicked() {
                            if let Err(error) = self.storage.delete(id) {
                                self.message(format!("Could not delete: {error}"));
                            }
                            self.projects = self.storage.list();
                            self.deleting_project = None;
                        }
                    });
                });
            if !open {
                self.deleting_project = None;
            }
        }
        if let Some((message, at)) = &self.toast {
            if at.elapsed() > Duration::from_secs(4) {
                self.toast = None;
            } else {
                egui::Area::new(Id::new("toast"))
                    .anchor(egui::Align2::CENTER_TOP, Vec2::new(0.0, 115.0))
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(CARD)
                            .corner_radius(8)
                            .stroke(Stroke::new(1.0_f32, LINE))
                            .inner_margin(11.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new(message).color(TEXT));
                            });
                    });
                ctx.request_repaint_after(Duration::from_millis(100));
            }
        }
    }
}

fn paint_wire(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    color: Color32,
    width: f32,
    active: bool,
) {
    let bend = ((end.x - start.x).abs() * 0.5).clamp(45.0, 120.0);
    let points = [
        start,
        start + Vec2::new(bend, 0.0),
        end - Vec2::new(bend, 0.0),
        end,
    ];
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
        points,
        false,
        Color32::TRANSPARENT,
        Stroke::new(width, color),
    ));
    if active {
        painter.circle_filled(end, 3.6, ACCENT_DETAIL);
    }
}

fn wire_point(start: Pos2, end: Pos2, t: f32) -> Pos2 {
    let bend = ((end.x - start.x).abs() * 0.5).clamp(45.0, 120.0);
    let control1 = start + Vec2::new(bend, 0.0);
    let control2 = end - Vec2::new(bend, 0.0);
    let one_minus_t = 1.0 - t;
    let point = start.to_vec2() * one_minus_t.powi(3)
        + control1.to_vec2() * (3.0 * one_minus_t.powi(2) * t)
        + control2.to_vec2() * (3.0 * one_minus_t * t * t)
        + end.to_vec2() * t.powi(3);
    Pos2::new(point.x, point.y)
}

fn distance_to_segment(point: Pos2, start: Pos2, end: Pos2) -> f32 {
    let segment = end - start;
    let length_sq = segment.length_sq();
    if length_sq <= f32::EPSILON {
        return point.distance(start);
    }
    let t = ((point - start).dot(segment) / length_sq).clamp(0.0, 1.0);
    point.distance(start + segment * t)
}

impl eframe::App for SkarvaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_connections(ctx);
        self.tick_run(ctx);
        if self
            .dirty_at
            .is_some_and(|at| at.elapsed() > Duration::from_millis(400))
        {
            self.save();
        }
        if self.project.is_none() {
            self.project_browser(ctx);
        } else {
            self.toolbar(ctx);
            if self.project.is_some() {
                self.library(ctx);
                self.inspector(ctx);
                self.run_panel(ctx);
                self.canvas_ui(ctx);
                let editing_text = ctx.wants_keyboard_input();
                if !editing_text && !self.dialog_open() && !self.run.running() {
                    let delete = ctx.input(|i| {
                        i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace)
                    });
                    if delete {
                        if self.selected_edge.is_some() {
                            self.delete_selected_edge();
                        } else {
                            self.delete_selected();
                        }
                    }
                    let duplicate =
                        ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::D));
                    if duplicate {
                        self.duplicate_selected();
                    }
                }
            }
        }
        self.dialogs(ctx);
    }

    fn on_exit(&mut self) {
        self.save();
    }
}

fn main() -> eframe::Result {
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--help" || argument == "-h" {
            println!(
                "Skarva — An agentic whiteboard\n\nUsage: skarva [--data-dir PATH] [--version]\n\n--data-dir PATH  Use a separate local profile (projects and encrypted vault).\n--version        Print the version.\n--help           Show this help."
            );
            return Ok(());
        } else if argument == "--version" || argument == "-V" {
            println!("Skarva {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        } else if argument == "--data-dir" {
            let Some(path) = arguments.next() else {
                eprintln!("--data-dir requires a path");
                std::process::exit(2);
            };
            let path = std::path::PathBuf::from(path);
            let path = if path.is_absolute() {
                path
            } else {
                match std::env::current_dir() {
                    Ok(directory) => directory.join(path),
                    Err(error) => {
                        eprintln!("Could not locate current directory: {error}");
                        std::process::exit(2);
                    }
                }
            };
            brand::set_data_directory(path);
        } else {
            eprintln!(
                "Unknown option: {}. Run skarva --help for usage.",
                argument.to_string_lossy()
            );
            std::process::exit(2);
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(brand::NAME)
            .with_app_id(brand::APP_ID)
            .with_icon(appearance::window_icon())
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        brand::NAME,
        options,
        Box::new(|cc| Ok(Box::new(SkarvaApp::new(cc)))),
    )
}

#[cfg(test)]
mod geometry_tests {
    use super::*;

    fn test_app(label: &str) -> (SkarvaApp, egui::Context) {
        let directory =
            std::env::temp_dir().join(format!("skarva-ui-{label}-{}", std::process::id()));
        let ctx = egui::Context::default();
        (SkarvaApp::with_storage(&ctx, Storage { directory }), ctx)
    }

    #[test]
    fn adding_blocks_avoids_overlap_and_dropping_respects_zoom() {
        let (mut app, _ctx) = test_app("placement");
        app.open_project(Project::new(100, "New workflow"));
        let start = app.add_node(NodeKind::Start, None).unwrap();
        let end = app.add_node(NodeKind::End, None).unwrap();
        let canvas = app.canvas().unwrap();
        assert!(canvas.node(end).unwrap().x >= canvas.node(start).unwrap().x + 300.0);
        app.zoom = 0.5;
        app.pan = Vec2::new(20.0, 30.0);
        let id = app
            .add_node(NodeKind::Word, Some(Pos2::new(200.0, 100.0)))
            .unwrap();
        let node = app.canvas().unwrap().node(id).unwrap();
        assert_eq!(
            Vec2::new(node.x + 124.0, node.y + 40.0),
            Vec2::new(420.0, 230.0)
        );
    }

    #[test]
    fn local_play_workflow_never_requests_credentials() {
        let (mut app, _ctx) = test_app("local");
        let mut project = Project::text_workflow(101, "Local workflow");
        project.canvases[0].nodes.remove(1);
        project.canvases[0].edges.remove(1);
        project.canvases[0].edges[0].to = 4;
        app.open_project(project);
        app.start_run();
        assert!(app.run.running());
        assert!(app.connection_receiver.is_none());
        assert!(!app.show_integrations);
    }

    #[test]
    fn yes_no_requires_both_answers_before_requesting_services() {
        let (mut app, _ctx) = test_app("yes-no");
        let mut project = Project::text_workflow(103, "Routing");
        project.canvases[0].nodes[1].kind = NodeKind::YesNo;
        app.open_project(project);
        app.start_run();
        assert!(!app.run.running());
        assert!(app.connection_receiver.is_none());
        assert!(app.toast.as_ref().unwrap().0.contains("both Yes and No"));
    }

    #[test]
    fn home_and_editor_render_at_minimum_window_size() {
        let (mut app, ctx) = test_app("layout");
        for editor in [false, true] {
            if editor {
                app.open_project(Project::text_workflow(102, "Writing"));
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(960.0, 620.0))),
                    ..Default::default()
                },
                |ctx| {
                    if editor {
                        app.toolbar(ctx);
                        app.library(ctx);
                        app.inspector(ctx);
                        app.run_panel(ctx);
                        app.canvas_ui(ctx);
                    } else {
                        app.project_browser(ctx);
                    }
                },
            );
            assert!(!output.shapes.is_empty());
        }
    }

    #[test]
    fn wire_curve_has_correct_endpoints() {
        let start = Pos2::new(12.0, 34.0);
        let end = Pos2::new(200.0, 80.0);
        assert_eq!(wire_point(start, end, 0.0), start);
        assert_eq!(wire_point(start, end, 1.0), end);
    }

    #[test]
    fn segment_distance_detects_nearby_points() {
        let start = Pos2::new(0.0, 0.0);
        let end = Pos2::new(100.0, 0.0);
        assert!(distance_to_segment(Pos2::new(50.0, 6.0), start, end) <= 7.0);
        assert!(distance_to_segment(Pos2::new(50.0, 20.0), start, end) > 7.0);
    }
}
