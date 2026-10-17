mod credentials;
mod jev;
mod model;
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
const ACCENT: Color32 = Color32::from_rgb(86, 210, 154);

fn kind_color(kind: NodeKind) -> Color32 {
    match kind {
        NodeKind::Start => Color32::from_rgb(139, 191, 164),
        NodeKind::Decision => Color32::from_rgb(205, 171, 119),
        NodeKind::Copilot => Color32::from_rgb(156, 148, 224),
        NodeKind::OpenAI => Color32::from_rgb(155, 195, 175),
        NodeKind::Teams => Color32::from_rgb(149, 157, 229),
        NodeKind::Outlook => Color32::from_rgb(118, 171, 223),
        NodeKind::Excel => Color32::from_rgb(111, 187, 139),
        NodeKind::Word => Color32::from_rgb(113, 155, 216),
        NodeKind::PowerPoint => Color32::from_rgb(212, 130, 100),
        NodeKind::Note => Color32::from_rgb(196, 174, 120),
        NodeKind::End => Color32::from_rgb(175, 180, 183),
    }
}

fn kind_icon(kind: NodeKind) -> &'static str {
    match kind {
        NodeKind::Start => ICON_PLAY_ARROW,
        NodeKind::Decision => ICON_ALT_ROUTE,
        NodeKind::Copilot => ICON_SMART_TOY,
        NodeKind::OpenAI => ICON_PSYCHOLOGY,
        NodeKind::Teams => ICON_FORUM,
        NodeKind::Outlook => ICON_MAIL,
        NodeKind::Excel => ICON_TABLE_CHART,
        NodeKind::Word => ICON_ARTICLE,
        NodeKind::PowerPoint => ICON_SLIDESHOW,
        NodeKind::Note => ICON_NOTE_ALT,
        NodeKind::End => ICON_STOP,
    }
}

fn category_icon(category: BlockCategory) -> &'static str {
    match category {
        BlockCategory::Flow => ICON_ACCOUNT_TREE,
        BlockCategory::Messages => ICON_CHAT_BUBBLE,
        BlockCategory::Ai => ICON_AUTO_AWESOME,
        BlockCategory::Office => ICON_GRID_VIEW,
    }
}

struct BlockPickerState {
    expanded: [bool; 4],
    search: String,
    focus_search: bool,
}

impl Default for BlockPickerState {
    fn default() -> Self {
        Self {
            expanded: [false; 4],
            search: String::new(),
            focus_search: false,
        }
    }
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
    }
}

fn picker_row(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    label: &str,
    indent: f32,
    active: bool,
    trailing: Option<&str>,
    color: Color32,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 34.0), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    if active || response.hovered() {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(0.0, 2.0)),
            6.0,
            if active {
                Color32::from_rgb(41, 48, 45)
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
                        category_icon(category),
                        category.label(),
                        0.0,
                        expanded,
                        Some(chevron),
                        MUTED,
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
                            kind_icon(kind),
                            kind.label(),
                            if query.is_empty() { 16.0 } else { 0.0 },
                            false,
                            None,
                            kind_color(kind),
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
    Preview,
    Jev,
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
}

impl MockRun {
    fn start(&mut self, node: u64, mode: RunMode, test_message: String) {
        *self = Self::default();
        self.mode = mode;
        self.test_message = test_message;
        self.queue.push_back(node);
        self.log.push(match mode {
            RunMode::Preview => "Preview started".into(),
            RunMode::Jev => "Jev live run started (integrations remain visual)".into(),
        });
        self.last_step = Some(Instant::now());
    }

    fn stop(&mut self) {
        self.active = None;
        self.queue.clear();
        self.pending = None;
        self.approval = None;
        self.paused = false;
        self.finished = true;
        self.log.push("Simulation stopped".into());
    }

    fn running(&self) -> bool {
        !self.finished
            && (self.active.is_some()
                || !self.queue.is_empty()
                || self.pending.is_some()
                || self.approval.is_some())
    }
}

struct StudioApp {
    storage: Storage,
    projects: Vec<Project>,
    project: Option<Project>,
    canvas_index: usize,
    selected: Option<u64>,
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
    show_new_canvas: bool,
    new_canvas_name: String,
    deleting_project: Option<u64>,
    show_run_log: bool,
    run: MockRun,
    run_mode: RunMode,
    test_input: String,
    show_jev_settings: bool,
    key_draft: Zeroizing<String>,
    key_present: Option<bool>,
    toast: Option<(String, Instant)>,
    dirty_at: Option<Instant>,
    pan_dragging: bool,
    node_drag: Option<(u64, Pos2, Vec2)>,
    block_menu_rect: Option<Rect>,
}

impl StudioApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_material_icons::initialize(&cc.egui_ctx);
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = PANEL;
        visuals.window_fill = PANEL;
        visuals.extreme_bg_color = BG;
        visuals.widgets.noninteractive.bg_fill = CARD;
        visuals.widgets.inactive.bg_fill = CARD;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(48, 52, 54);
        visuals.widgets.active.bg_fill = Color32::from_rgb(56, 64, 61);
        visuals.widgets.inactive.fg_stroke.color = TEXT;
        visuals.widgets.hovered.fg_stroke.color = TEXT;
        visuals.widgets.active.fg_stroke.color = TEXT;
        visuals.selection.bg_fill = Color32::from_rgb(53, 86, 72);
        visuals.selection.stroke.color = ACCENT;
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|style| {
            style.spacing.item_spacing = Vec2::new(8.0, 8.0);
            style.spacing.button_padding = Vec2::new(11.0, 7.0);
        });
        let storage = Storage::new();
        let projects = storage.list();
        Self {
            storage,
            projects,
            project: None,
            canvas_index: 0,
            selected: None,
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
            show_new_canvas: false,
            new_canvas_name: String::new(),
            deleting_project: None,
            show_run_log: true,
            run: MockRun::default(),
            run_mode: RunMode::Preview,
            test_input: String::new(),
            show_jev_settings: false,
            key_draft: Zeroizing::new(String::new()),
            key_present: None,
            toast: None,
            dirty_at: None,
            pan_dragging: false,
            node_drag: None,
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
        if let Some(project) = &self.project {
            if let Err(error) = self.storage.save(project) {
                self.message(format!("Could not save: {error}"));
            }
        }
        self.dirty_at = None;
    }

    fn open_project(&mut self, project: Project) {
        self.test_input.zeroize();
        self.project = Some(project);
        self.canvas_index = 0;
        self.selected = None;
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
        let project = Project::new(id, name);
        self.open_project(project);
        self.save();
        self.show_new_project = false;
        self.new_project_name.clear();
    }

    fn add_node(&mut self, kind: NodeKind, center: Option<Pos2>) -> Option<u64> {
        let count = self.canvas().map(|c| c.nodes.len()).unwrap_or(0);
        let pos = if let Some(center) = center {
            Vec2::new(
                (center.x - 124.0) / self.zoom + self.pan.x,
                (center.y - 40.0) / self.zoom + self.pan.y,
            )
        } else {
            Vec2::new(
                self.pan.x - 410.0 + (count % 3) as f32 * 285.0,
                self.pan.y - 60.0 + (count / 3) as f32 * 170.0,
            )
        };
        let project = self.project.as_mut()?;
        let id = project.allocate_id();
        if let Some(canvas) = project.canvases.get_mut(self.canvas_index) {
            canvas.nodes.push(Node::new(id, kind, pos.x, pos.y));
            self.selected = Some(id);
            self.changed();
            return Some(id);
        }
        None
    }

    fn delete_selected(&mut self) {
        let Some(id) = self.selected.take() else {
            return;
        };
        if let Some(canvas) = self.canvas_mut() {
            canvas.nodes.retain(|n| n.id != id);
            canvas.edges.retain(|e| e.from != id && e.to != id);
            self.changed();
        }
    }

    fn delete_selected_edge(&mut self) {
        let Some(id) = self.selected_edge.take() else {
            return;
        };
        if let Some(canvas) = self.canvas_mut() {
            canvas.edges.retain(|edge| edge.id != id);
            self.changed();
        }
    }

    fn duplicate_selected(&mut self) {
        let Some(id) = self.selected else {
            return;
        };
        let Some(mut copy) = self.canvas().and_then(|c| c.node(id)).cloned() else {
            return;
        };
        let Some(project) = self.project.as_mut() else {
            return;
        };
        copy.id = project.allocate_id();
        copy.x += 32.0;
        copy.y += 32.0;
        let new_id = copy.id;
        project.canvases[self.canvas_index].nodes.push(copy);
        self.selected = Some(new_id);
        self.changed();
    }

    fn connect_to(&mut self, to: u64) {
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
            if canvas
                .node(from)
                .is_some_and(|node| node.kind == NodeKind::Decision)
            {
                canvas
                    .edges
                    .retain(|edge| edge.from != from || edge.output != output);
            }
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
        if self.run.approval.is_some() {
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
                        if node.kind == NodeKind::Decision {
                            format!(
                                "{} → {}",
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
                            description: if node.kind == NodeKind::Decision {
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
                self.run.visited.insert(id);
                self.run.path_titles.push(node.title.clone());
                self.run.log.push(format!("Visited {}", node.title));
                if self.run.mode == RunMode::Jev && outgoing.len() > 1 {
                    let request = jev::RouteRequest {
                        test_message: self.run.test_message.clone(),
                        current_block: format!("{} ({})", node.title, node.kind.label()),
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
                let chosen = if self.run.mode == RunMode::Jev && outgoing.len() == 1 {
                    outgoing.first().cloned()
                } else if node.kind == NodeKind::Decision {
                    outgoing
                        .iter()
                        .find(|edge| edge.output == node.mock_route)
                        .cloned()
                } else if outgoing.len() > 1 {
                    outgoing
                        .get(node.mock_route.min(outgoing.len() - 1))
                        .cloned()
                } else {
                    outgoing.first().cloned()
                };
                if self.run.mode == RunMode::Preview
                    && (node.kind == NodeKind::Decision || outgoing.len() > 1)
                {
                    if let Some(edge) = &chosen {
                        let index = outgoing
                            .iter()
                            .position(|candidate| candidate.id == edge.id)
                            .unwrap_or(0);
                        self.run
                            .log
                            .push(format!("Preview chose {}", labels[index]));
                    }
                }
                if let Some(edge) = chosen {
                    self.follow_run_edge(edge);
                }
            }
        } else {
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

    fn start_run(&mut self) {
        if self.run_mode == RunMode::Jev && self.key_present == Some(false) {
            self.show_jev_settings = true;
            self.message("Save a TypeSafe API key in Jev settings first");
            return;
        }
        if self.run_mode == RunMode::Jev && self.test_input.trim().is_empty() {
            self.message("Enter a test message in the Run Log before using Jev");
            self.show_run_log = true;
            return;
        }
        let Some(canvas) = self.canvas() else {
            return;
        };
        let starts = canvas
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Start)
            .map(|n| n.id)
            .collect::<Vec<_>>();
        if starts.is_empty() {
            self.message("Add a Start block to play this canvas");
            return;
        }
        let start = self
            .selected
            .filter(|id| starts.contains(id))
            .unwrap_or(starts[0]);
        self.run
            .start(start, self.run_mode, self.test_input.trim().to_owned());
        self.show_run_log = true;
    }

    fn project_browser(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("browser_top")
            .exact_height(68.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(24.0);
                    ui.label(RichText::new("JEV").size(20.0).strong().color(TEXT));
                    ui.label(RichText::new("STUDIO").size(12.0).strong().color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(22.0);
                        ui.label(
                            RichText::new("NATIVE CANVAS PROTOTYPE")
                                .size(10.0)
                                .color(MUTED),
                        );
                    });
                });
            });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(55.0);
            ui.horizontal(|ui| {
                ui.add_space(58.0);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Your workspace")
                            .size(32.0)
                            .strong()
                            .color(TEXT),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("Build ideas into connected, runnable canvases.")
                            .size(14.0)
                            .color(MUTED),
                    );
                    ui.add_space(28.0);
                    if ui
                        .add(
                            egui::Button::new(RichText::new("New project").size(14.0).color(BG))
                                .fill(ACCENT)
                                .corner_radius(9),
                        )
                        .clicked()
                    {
                        self.show_new_project = true;
                    }
                    ui.add_space(46.0);
                    ui.label(RichText::new("PROJECTS").size(11.0).strong().color(MUTED));
                    ui.add_space(13.0);
                    if self.projects.is_empty() {
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width().min(760.0), 180.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 14, CARD);
                        ui.painter().rect_stroke(
                            rect,
                            14,
                            Stroke::new(1.0_f32, LINE),
                            StrokeKind::Inside,
                        );
                        ui.painter().text(
                            rect.center() - Vec2::new(0.0, 13.0),
                            egui::Align2::CENTER_CENTER,
                            "A place for your first idea",
                            FontId::proportional(17.0),
                            TEXT,
                        );
                        ui.painter().text(
                            rect.center() + Vec2::new(0.0, 17.0),
                            egui::Align2::CENTER_CENTER,
                            "Create a project to open an empty canvas.",
                            FontId::proportional(13.0),
                            MUTED,
                        );
                    } else {
                        let projects = self.projects.clone();
                        for project in projects {
                            let label = format!(
                                "{}     ·     {} canvas{}",
                                project.name,
                                project.canvases.len(),
                                if project.canvases.len() == 1 {
                                    ""
                                } else {
                                    "es"
                                }
                            );
                            ui.horizontal(|ui| {
                                if ui
                                    .add_sized(
                                        [ui.available_width().min(660.0), 68.0],
                                        egui::Button::new(RichText::new(label).size(15.0))
                                            .fill(CARD)
                                            .corner_radius(10),
                                    )
                                    .clicked()
                                {
                                    self.open_project(project.clone());
                                }
                                if ui
                                    .add_sized(
                                        [80.0, 36.0],
                                        egui::Button::new("Delete").fill(PANEL).frame(false),
                                    )
                                    .clicked()
                                {
                                    self.deleting_project = Some(project.id);
                                }
                            });
                        }
                    }
                });
            });
        });
    }

    fn toolbar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top")
            .exact_height(66.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(14.0);
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(ICON_ARROW_BACK).size(18.0).color(MUTED),
                            )
                            .fill(PANEL)
                            .frame(false),
                        )
                        .on_hover_text("Projects")
                        .clicked()
                    {
                        self.save();
                        self.projects = self.storage.list();
                        self.project = None;
                        self.run = MockRun::default();
                        self.test_input.zeroize();
                        return;
                    }
                    ui.add_space(8.0);
                    ui.label(RichText::new("JEV").size(17.0).strong().color(TEXT));
                    ui.add_space(17.0);
                    ui.separator();
                    ui.add_space(10.0);
                    if let Some(project) = &self.project {
                        ui.label(RichText::new(&project.name).size(15.0).strong().color(TEXT));
                    }
                    ui.add_space(14.0);
                    ui.label(RichText::new("/  Agentic canvas").size(13.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(14.0);
                        if ui
                            .add_sized(
                                [82.0, 38.0],
                                egui::Button::new(RichText::new("Reset").color(TEXT))
                                    .fill(CARD)
                                    .corner_radius(8),
                            )
                            .on_hover_text("Clear the run and its highlighted path")
                            .clicked()
                        {
                            self.run = MockRun::default();
                        }
                        if ui
                            .add_sized(
                                [88.0, 38.0],
                                egui::Button::new(
                                    RichText::new(if self.run.paused { "Resume" } else { "Pause" })
                                        .color(TEXT),
                                )
                                .fill(CARD)
                                .corner_radius(8),
                            )
                            .clicked()
                        {
                            if self.run.running() && self.run.approval.is_none() {
                                self.run.paused = !self.run.paused;
                            }
                        }
                        if ui
                            .add_sized(
                                [104.0, 38.0],
                                egui::Button::new(
                                    RichText::new(format!("{}  Play", ICON_PLAY_ARROW))
                                        .color(BG)
                                        .strong(),
                                )
                                .fill(ACCENT)
                                .corner_radius(8),
                            )
                            .clicked()
                        {
                            self.start_run();
                        }
                        ui.add_space(9.0);
                        ui.scope(|ui| {
                            if self.run.running() {
                                ui.disable();
                            }
                            ui.spacing_mut().interact_size.y = 38.0;
                            ui.visuals_mut().widgets.inactive.corner_radius = CornerRadius::same(8);
                            ui.visuals_mut().widgets.hovered.corner_radius = CornerRadius::same(8);
                            ui.visuals_mut().widgets.active.corner_radius = CornerRadius::same(8);
                            ui.visuals_mut().widgets.open.corner_radius = CornerRadius::same(8);
                            egui::ComboBox::from_id_salt("run_mode")
                                .width(176.0)
                                .selected_text(match self.run_mode {
                                    RunMode::Preview => "Preview",
                                    RunMode::Jev => "Jev live",
                                })
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        &mut self.run_mode,
                                        RunMode::Preview,
                                        "Preview · no API calls",
                                    );
                                    ui.selectable_value(
                                        &mut self.run_mode,
                                        RunMode::Jev,
                                        "Jev live · routes branches",
                                    );
                                });
                        });
                        if ui
                            .add_sized(
                                [154.0, 38.0],
                                egui::Button::new(format!("{}  Jev settings", ICON_KEY)),
                            )
                            .on_hover_text("Enter your TypeSafe API key")
                            .clicked()
                        {
                            self.show_jev_settings = true;
                        }
                    });
                });
            });
        egui::TopBottomPanel::top("tabs")
            .exact_height(39.0)
            .show(ctx, |ui| {
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
                            self.canvas_index = index;
                            self.selected = None;
                            self.selected_edge = None;
                            self.connecting = None;
                            self.connection_dragging = false;
                            self.connection_menu = None;
                            self.context_menu_canvas_pos = None;
                            self.menu_picker = BlockPickerState::default();
                            self.run = MockRun::default();
                            self.pan = Vec2::ZERO;
                            self.zoom = 1.0;
                        }
                    }
                    if ui
                        .add(
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
                ui.add_space(17.0);
                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.label(RichText::new("BLOCKS").size(11.0).strong().color(MUTED));
                });
                ui.add_space(13.0);
                ui.separator();
                ui.add_space(12.0);
                let picker_height = (ui.available_height() - 48.0).max(220.0);
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
                ui.add_space(17.0);
                ui.horizontal(|ui| {
                    ui.add_space(15.0);
                    ui.label(RichText::new("INSPECTOR").size(11.0).strong().color(MUTED));
                });
                ui.add_space(16.0);
                ui.separator();
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
                            ui.label(RichText::new(format!("{from} → {to}")).size(13.0).color(TEXT));
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
                        ui.label(RichText::new("PROTOTYPE BLOCK").size(10.0).color(MUTED));
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
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(RichText::new("Subtitle").size(12.0).color(MUTED));
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
                    if node.kind == NodeKind::Decision {
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new("ROUTES").size(11.0).strong().color(MUTED));
                        });
                        ui.add_space(8.0);
                        for index in 0..node.routes.len() {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(format!("{}", index + 1))
                                        .size(11.0)
                                        .color(MUTED),
                                );
                                changed |= ui
                                    .add_sized(
                                        [225.0, 27.0],
                                        egui::TextEdit::singleline(&mut node.routes[index]),
                                    )
                                    .changed();
                            });
                        }
                        if node.routes.len() < 6 {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                if ui.small_button("+ Add route").clicked() {
                                    node.routes.push(format!("Route {}", node.routes.len() + 1));
                                    changed = true;
                                }
                            });
                        }
                        if node.routes.len() > 1 {
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                if ui.small_button("Remove last route").clicked() {
                                    remove_last_route = true;
                                }
                            });
                        }
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("SIMULATED CHOICE")
                                    .size(11.0)
                                    .strong()
                                    .color(MUTED),
                            );
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            egui::ComboBox::from_id_salt(("route", id))
                                .selected_text(node.output_label(node.mock_route))
                                .show_ui(ui, |ui| {
                                    for (index, route) in node.routes.iter().enumerate() {
                                        changed |= ui
                                            .selectable_value(&mut node.mock_route, index, route)
                                            .changed();
                                    }
                                });
                        });
                        ui.add_space(7.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                RichText::new("The simulated run follows this route.")
                                    .size(11.0)
                                    .color(MUTED),
                            );
                        });
                    } else if branch_targets.len() > 1 {
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new("SIMULATED BRANCH").size(11.0).strong().color(MUTED));
                        });
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            egui::ComboBox::from_id_salt(("branch", id))
                                .selected_text(&branch_targets[node.mock_route.min(branch_targets.len() - 1)])
                                .show_ui(ui, |ui| {
                                    for (index, target) in branch_targets.iter().enumerate() {
                                        changed |= ui.selectable_value(&mut node.mock_route, index, target).changed();
                                    }
                                });
                        });
                        ui.add_space(7.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.add_sized(
                                [255.0, 35.0],
                                egui::Label::new(RichText::new("Preview uses this choice. Jev live chooses from the connected blocks.").size(11.0).color(MUTED)).wrap(),
                            );
                        });
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            egui::CollapsingHeader::new("Jev request preview")
                                .id_salt(("routing_prompt", id))
                                .show(ui, |ui| {
                                    let prompt = format!(
                                        "State: test message, current block {}, and visited path. Choice: which connected path best handles the request? Options: {}. Below {:.0}% confidence, pause for human approval.",
                                        node.title,
                                        branch_targets.join(", "),
                                        node.threshold * 100.0,
                                    );
                                    ui.add_sized([240.0, 100.0], egui::Label::new(RichText::new(prompt).size(11.0).color(MUTED)).wrap());
                                });
                        });
                    }
                    if node.kind == NodeKind::Decision || branch_targets.len() > 1 {
                        ui.add_space(17.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new("JEV CONFIDENCE GATE").size(11.0).strong().color(MUTED));
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            changed |= ui.add_sized(
                                [252.0, 24.0],
                                egui::Slider::new(&mut node.threshold, 0.0..=1.0)
                                    .show_value(true)
                            ).changed();
                        });
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(RichText::new("Below this, pause for human approval.").size(11.0).color(MUTED));
                        });
                    }
                }
                if remove_last_route {
                    if let Some(canvas) = self.canvas_mut() {
                        if let Some(node) = canvas.node_mut(id) {
                            node.routes.pop();
                            node.mock_route =
                                node.mock_route.min(node.routes.len().saturating_sub(1));
                            let count = node.routes.len();
                            canvas
                                .edges
                                .retain(|edge| edge.from != id || edge.output < count);
                        }
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
                                    format!("{}  {}", if e.from == id { "→" } else { "←" }, name),
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
                        {
                            if let Some(canvas) = self.canvas_mut() {
                                canvas.edges.retain(|e| e.id != edge_id);
                                self.changed();
                            }
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
    }

    fn run_panel(&mut self, ctx: &egui::Context) {
        let height = if !self.show_run_log {
            32.0
        } else if self.run.approval.is_some() {
            238.0
        } else if self.run_mode == RunMode::Jev {
            195.0
        } else {
            146.0
        };
        egui::TopBottomPanel::bottom("run_log")
            .exact_height(height)
            .show(ctx, |ui| {
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
                    ui.label(RichText::new("RUN LOG").size(11.0).strong().color(MUTED));
                    ui.add_space(9.0);
                    let status = if self.run.approval.is_some() {
                        "Needs approval"
                    } else if self.run.pending.is_some() {
                        "Waiting for Jev"
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
                            .color(if self.run.running() { ACCENT } else { MUTED }),
                    );
                    ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                        ui.add_space(13.0);
                        ui.label(
                            RichText::new(if self.run_mode == RunMode::Jev {
                                "Jev calls only at branches · no app integrations"
                            } else {
                                "Preview only · no API calls"
                            })
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                });
                if self.show_run_log {
                    if self.run_mode == RunMode::Jev {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(RichText::new("TEST MESSAGE").size(10.0).strong().color(MUTED));
                            ui.add_enabled(
                                !self.run.running(),
                                egui::TextEdit::singleline(&mut self.test_input)
                                    .desired_width(ui.available_width() - 20.0)
                                    .hint_text("Type a sample message for Jev to route"),
                            );
                        });
                    }
                    let mut approved = None;
                    if let Some(approval) = &self.run.approval {
                        ui.horizontal(|ui| {
                            ui.add_space(16.0);
                            ui.label(RichText::new(format!(
                                "Jev confidence {:.0}% is below {:.0}%. Choose a path:",
                                approval.confidence * 100.0,
                                approval.threshold * 100.0,
                            )).size(12.0).color(TEXT));
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
                                        RichText::new(
                                            "Press Play to follow the canvas. Jev live uses the test message above.",
                                        )
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
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(BG))
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let response = ui.interact(rect, Id::new("canvas_background"), Sense::click());
                ui.painter().rect_filled(rect, 0, BG);
                self.draw_grid(ui, rect);
                let pointer = ui.input(|i| i.pointer.hover_pos());
                let hit_node = pointer.is_some_and(|p| {
                    self.canvas().is_some_and(|c| {
                        c.nodes.iter().any(|n| self.node_rect(rect, n).contains(p))
                    })
                });
                let pressed =
                    self.connection_menu.is_none() && ui.input(|i| i.pointer.primary_pressed());
                let down = ui.input(|i| i.pointer.primary_down());
                let edge_hit = if pressed || response.clicked() {
                    pointer
                        .filter(|p| rect.contains(*p) && !hit_node)
                        .and_then(|p| self.hit_edge(rect, p))
                } else {
                    None
                };
                if pressed {
                    if let Some((from, output)) = pointer.and_then(|p| self.hit_output(rect, p)) {
                        self.connecting = Some((from, output));
                        self.connection_dragging = true;
                        self.connection_menu = None;
                        self.selected = Some(from);
                        self.selected_edge = None;
                    } else if let Some(edge_id) = edge_hit {
                        self.selected_edge = Some(edge_id);
                        self.selected = None;
                        self.connecting = None;
                    }
                    self.node_drag = pointer.and_then(|p| {
                        if self.connection_dragging {
                            return None;
                        }
                        self.canvas()?.nodes.iter().rev().find_map(|node| {
                            let body = self
                                .node_rect(rect, node)
                                .shrink2(Vec2::new(17.0 * self.zoom, 6.0 * self.zoom));
                            body.contains(p)
                                .then_some((node.id, p, Vec2::new(node.x, node.y)))
                        })
                    });
                }
                if let (Some((id, origin, initial)), Some(p)) = (self.node_drag, pointer) {
                    let offset = (p - origin) / self.zoom;
                    if offset.length_sq() > 4.0 {
                        if let Some(node) = self.canvas_mut().and_then(|c| c.node_mut(id)) {
                            node.x = initial.x + offset.x;
                            node.y = initial.y + offset.y;
                            self.selected = Some(id);
                            self.selected_edge = None;
                            self.changed();
                        }
                    }
                }
                if !down {
                    self.node_drag = None;
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
                        self.zoom = (self.zoom * factor).clamp(0.35, 2.5);
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
                if let Some((from, output)) = self.connecting {
                    if let Some(node) = self.canvas().and_then(|c| c.node(from)) {
                        let start = self.output_pos(rect, node, output);
                        let end = ui.input(|i| i.pointer.hover_pos()).unwrap_or(start);
                        paint_wire(ui.painter(), start, end, ACCENT, 2.0, false);
                    }
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
                let zoom_response = ui.interact(controls, Id::new("zoom_reset"), Sense::click());
                ui.painter().text(
                    controls.center(),
                    egui::Align2::CENTER_CENTER,
                    zoom_label,
                    FontId::proportional(12.0),
                    TEXT,
                );
                if zoom_response.clicked() {
                    self.zoom = 1.0;
                    self.pan = Vec2::ZERO;
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
        let height = if node.kind == NodeKind::Decision {
            70.0 + node.output_count() as f32 * 18.0
        } else {
            82.0
        };
        Rect::from_min_size(pos, Vec2::new(250.0, height) * self.zoom)
    }

    fn output_pos(&self, canvas_rect: Rect, node: &Node, output: usize) -> Pos2 {
        let rect = self.node_rect(canvas_rect, node);
        if node.kind == NodeKind::Decision {
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
                    ACCENT
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
            let selected = self.selected == Some(node.id);
            let active = self.run.active == Some(node.id) && self.run.running();
            let visited = self.run.visited.contains(&node.id);
            ui.painter().rect_filled(rect, CornerRadius::same(12), CARD);
            let border = if active {
                ACCENT
            } else if selected {
                Color32::from_rgb(193, 217, 204)
            } else if visited {
                Color32::from_rgb(91, 135, 110)
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
            if node.kind == NodeKind::Decision {
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
                        ACCENT
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
                            ACCENT
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
                            ACCENT
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
                {
                    self.connecting = Some((node.id, output));
                    self.selected = Some(node.id);
                    self.selected_edge = None;
                }
            }
            let body = rect.shrink2(Vec2::new(17.0 * self.zoom, 6.0 * self.zoom));
            let response = ui.interact(body, Id::new(("node", node.id)), Sense::click_and_drag());
            if response.clicked() {
                self.selected = Some(node.id);
                self.selected_edge = None;
                self.connecting = None;
            }
            response.context_menu(|ui| {
                if ui.button("Duplicate").clicked() {
                    self.selected = Some(node.id);
                    self.duplicate_selected();
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    self.selected = Some(node.id);
                    self.delete_selected();
                    ui.close();
                }
            });
        }
        if self.connecting.is_some() && pointer.is_some_and(|p| !canvas_rect.contains(p)) {
            self.connecting = None;
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.show_jev_settings {
            if self.key_present.is_none() {
                match credentials::load() {
                    Ok(Some(mut key)) => {
                        key.zeroize();
                        self.key_present = Some(true);
                    }
                    Ok(None) => self.key_present = Some(false),
                    Err(error) => {
                        self.message(format!("Could not access credential store: {error}"));
                        self.key_present = Some(false);
                    }
                }
            }
            let mut open = true;
            egui::Window::new("Jev settings")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .default_width(390.0)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.label(RichText::new("TypeSafe API key").size(13.0).color(TEXT));
                    ui.add_space(4.0);
                    ui.add_sized(
                        [360.0, 31.0],
                        egui::TextEdit::singleline(&mut *self.key_draft)
                            .password(true)
                            .hint_text("Paste key from console.typesafe.ai"),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(if self.key_present == Some(true) {
                            "A key is saved in your OS credential store."
                        } else {
                            "No key saved yet."
                        })
                        .size(12.0)
                        .color(MUTED),
                    );
                    ui.label(
                        RichText::new("The key is never saved in a project or exported canvas.")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.label(
                        RichText::new("Jev live sends the test message and connected block details to TypeSafe.")
                            .size(11.0)
                            .color(MUTED),
                    );
                    ui.add_space(13.0);
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Button::new("Save key").fill(ACCENT))
                            .clicked()
                        {
                            match credentials::save(&self.key_draft) {
                                Ok(()) => {
                                    self.key_draft.zeroize();
                                    self.key_present = Some(true);
                                    self.message("Jev API key saved securely");
                                }
                                Err(error) => self.message(format!("Could not save key: {error}")),
                            }
                        }
                        if self.key_present == Some(true) && ui.button("Remove saved key").clicked()
                        {
                            match credentials::remove() {
                                Ok(()) => {
                                    self.key_present = Some(false);
                                    self.message("Jev API key removed");
                                }
                                Err(error) => {
                                    self.message(format!("Could not remove key: {error}"))
                                }
                            }
                        }
                    });
                });
            self.show_jev_settings &= open;
            if !self.show_jev_settings {
                self.key_draft.zeroize();
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
                            .hint_text("e.g. Project assistant"),
                    );
                    response.request_focus();
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(10.0);
                    if ui
                        .add(
                            egui::Button::new("Create project")
                                .fill(ACCENT)
                                .corner_radius(7),
                        )
                        .clicked()
                        || enter
                    {
                        self.create_project();
                    }
                });
            self.show_new_project &= open;
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
                    response.request_focus();
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.add_space(10.0);
                    if ui
                        .add(
                            egui::Button::new("Create canvas")
                                .fill(ACCENT)
                                .corner_radius(7),
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
        painter.circle_filled(end, 3.6, ACCENT);
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

#[cfg(test)]
mod geometry_tests {
    use super::*;

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

impl eframe::App for StudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                if !editing_text {
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("JEV Studio")
            .with_inner_size([1440.0, 900.0])
            .with_min_inner_size([960.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "JEV Studio",
        options,
        Box::new(|cc| Ok(Box::new(StudioApp::new(cc)))),
    )
}
