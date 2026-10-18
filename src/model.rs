use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Start,
    Decision,
    YesNo,
    Copilot,
    OpenAI,
    Teams,
    Outlook,
    Excel,
    Word,
    PowerPoint,
    Note,
    End,
    GmailInbox,
    GmailReply,
    WebSearch,
    LocalFile,
    Recall,
    Remember,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCategory {
    Flow,
    Messages,
    Ai,
    Office,
    Files,
}

impl BlockCategory {
    pub const ALL: [Self; 5] = [
        Self::Flow,
        Self::Messages,
        Self::Ai,
        Self::Office,
        Self::Files,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flow => "Flow & canvas",
            Self::Messages => "Messages",
            Self::Ai => "AI & assistants",
            Self::Office => "Office apps",
            Self::Files => "Files & data",
        }
    }
}

impl NodeKind {
    pub const ALL: [Self; 13] = [
        Self::Start,
        Self::Decision,
        Self::YesNo,
        Self::OpenAI,
        Self::Excel,
        Self::Word,
        Self::PowerPoint,
        Self::Note,
        Self::End,
        Self::GmailInbox,
        Self::GmailReply,
        Self::WebSearch,
        Self::LocalFile,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Play",
            Self::Decision => "Choice",
            Self::YesNo => "Yes / No",
            Self::Copilot => "Copilot",
            Self::OpenAI => "AI model",
            Self::Teams => "Teams",
            Self::Outlook => "Outlook",
            Self::Excel => "Excel",
            Self::Word => "Word",
            Self::PowerPoint => "PowerPoint",
            Self::Note => "Note",
            Self::End => "End",
            Self::GmailInbox => "Email received",
            Self::GmailReply => "Reply to email",
            Self::WebSearch => "Web research",
            Self::LocalFile => "Local file",
            Self::Recall => "Recall",
            Self::Remember => "Remember",
        }
    }

    pub fn category(self) -> BlockCategory {
        match self {
            Self::Start | Self::Decision | Self::YesNo | Self::Note | Self::End => {
                BlockCategory::Flow
            }
            Self::Teams | Self::Outlook | Self::GmailInbox | Self::GmailReply => {
                BlockCategory::Messages
            }
            Self::Copilot | Self::OpenAI | Self::WebSearch => BlockCategory::Ai,
            Self::Excel | Self::Word | Self::PowerPoint => BlockCategory::Office,
            Self::LocalFile | Self::Recall | Self::Remember => BlockCategory::Files,
        }
    }

    pub fn picker_detail(self) -> &'static str {
        match self {
            Self::Start => "Manual entry point",
            Self::Decision => "Choose among named routes",
            Self::YesNo => "Answer a yes or no question",
            Self::Note => "Canvas annotation",
            Self::End => "Finish a path",
            Self::Copilot => "Integration unavailable",
            Self::OpenAI => "Draft or transform text",
            Self::Teams => "Integration unavailable",
            Self::Outlook => "Integration unavailable",
            Self::Excel => "Create an editable spreadsheet",
            Self::Word => "Create an editable document",
            Self::PowerPoint => "Create an editable presentation",
            Self::GmailInbox => "New unread email trigger",
            Self::GmailReply => "Reply after approval",
            Self::WebSearch => "Find current sources for the request",
            Self::LocalFile => "Read a selected file on this computer",
            Self::Recall => "Load project memory by key",
            Self::Remember => "Save last output in project memory",
        }
    }

    pub fn default_subtitle(self) -> &'static str {
        match self {
            Self::Start => "Run with a starting message",
            Self::Decision => "Choose a route",
            Self::YesNo => "Yes or no",
            Self::Copilot => "Integration unavailable",
            Self::OpenAI => "Configured LLM provider",
            Self::Teams => "Integration unavailable",
            Self::Outlook => "Integration unavailable",
            Self::Excel => "Generate an XLSX file",
            Self::Word => "Generate a DOCX file",
            Self::PowerPoint => "Generate a PPTX file",
            Self::Note => "Canvas annotation",
            Self::End => "Finish run",
            Self::GmailInbox => "Wait for unread mail",
            Self::GmailReply => "Send approved reply",
            Self::WebSearch => "Search web and pass sources onward",
            Self::LocalFile => "Read a local document",
            Self::Recall => "Look up an earlier result",
            Self::Remember => "Save a result for later runs",
        }
    }

    pub fn outputs(self) -> &'static [&'static str] {
        match self {
            Self::Decision => &["Route A", "Route B", "Route C"],
            Self::YesNo => &["Yes", "No"],
            Self::End | Self::Note => &[],
            _ => &["Next"],
        }
    }

    pub fn accepts_input(self) -> bool {
        !matches!(self, Self::Start | Self::Note | Self::GmailInbox)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: u64,
    pub kind: NodeKind,
    pub title: String,
    pub subtitle: String,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub mock_route: usize,
    #[serde(default)]
    pub mock_target: usize,
    #[serde(default)]
    pub routes: Vec<String>,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub decision_question: String,
}

fn default_threshold() -> f32 {
    0.7
}

impl Node {
    pub fn new(id: u64, kind: NodeKind, x: f32, y: f32) -> Self {
        let routes = if kind == NodeKind::Decision {
            vec!["Route A".into(), "Route B".into(), "Route C".into()]
        } else {
            Vec::new()
        };
        Self {
            id,
            kind,
            title: kind.label().into(),
            subtitle: kind.default_subtitle().into(),
            x,
            y,
            mock_route: 0,
            mock_target: 0,
            routes,
            threshold: default_threshold(),
            prompt: match kind {
                NodeKind::OpenAI => "Respond to the incoming request clearly and concisely. Do not invent facts. Ask for clarification when needed.".into(),
                NodeKind::Word => "Create a useful document that directly answers the incoming request.".into(),
                NodeKind::Excel => "Organize the requested information into a useful table with clear headings.".into(),
                NodeKind::PowerPoint => "Create a concise presentation that directly answers the incoming request.".into(),
                NodeKind::WebSearch => "Find reliable sources relevant to the incoming request.".into(),
                NodeKind::Recall | NodeKind::Remember => "{sender}".into(),
                _ => String::new(),
            },
            decision_question: String::new(),
        }
    }

    pub fn routing_question(&self) -> String {
        let explicit = self.decision_question.trim();
        if !explicit.is_empty() {
            return explicit.to_owned();
        }
        if matches!(self.kind, NodeKind::Decision | NodeKind::YesNo)
            && !self.title.trim().is_empty()
            && self.title.trim() != self.kind.label()
            && self.title.trim() != "Decision"
        {
            return self.title.trim().to_owned();
        }
        "Which connected path best handles this message?".into()
    }

    pub fn output_count(&self) -> usize {
        if self.kind == NodeKind::Decision {
            self.routes.len().max(1)
        } else {
            self.kind.outputs().len()
        }
    }

    pub fn output_label(&self, index: usize) -> &str {
        if self.kind == NodeKind::Decision {
            self.routes
                .get(index)
                .map(String::as_str)
                .unwrap_or("Route A")
        } else {
            self.kind.outputs().get(index).copied().unwrap_or("Next")
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: u64,
    pub from: u64,
    pub output: usize,
    pub to: u64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub id: u64,
    pub name: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

impl Canvas {
    pub fn validate(&self) -> Result<(), String> {
        use std::collections::{HashMap, HashSet, VecDeque};
        let ids: HashSet<_> = self.nodes.iter().map(|n| n.id).collect();
        if ids.len() != self.nodes.len() {
            return Err("A canvas contains duplicate block IDs".into());
        }
        let mut indegree: HashMap<_, usize> = ids.iter().map(|id| (*id, 0)).collect();
        let mut connections = HashSet::new();
        for node in &self.nodes {
            if !node.x.is_finite() || !node.y.is_finite() || !(0.0..=1.0).contains(&node.threshold)
            {
                return Err("A block has invalid position or confidence settings".into());
            }
        }
        for edge in &self.edges {
            let from = self
                .node(edge.from)
                .ok_or("A connection starts at a missing block")?;
            let to = self
                .node(edge.to)
                .ok_or("A connection ends at a missing block")?;
            if edge.from == edge.to
                || edge.output >= from.output_count()
                || !to.kind.accepts_input()
            {
                return Err("A connection uses an invalid block port".into());
            }
            if !connections.insert((edge.from, edge.output, edge.to)) {
                return Err("A canvas contains a duplicate connection".into());
            }
            *indegree.get_mut(&edge.to).unwrap() += 1;
        }
        let mut ready: VecDeque<_> = indegree
            .iter()
            .filter(|(_, degree)| **degree == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut count = 0;
        while let Some(id) = ready.pop_front() {
            count += 1;
            for edge in self.edges.iter().filter(|e| e.from == id) {
                let degree = indegree.get_mut(&edge.to).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    ready.push_back(edge.to);
                }
            }
        }
        if count != self.nodes.len() {
            return Err("This workflow contains a loop. Remove the connection that leads back to an earlier block.".into());
        }
        Ok(())
    }

    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    pub fn node_mut(&mut self, id: u64) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub id: u64,
    pub name: String,
    pub canvases: Vec<Canvas>,
    pub next_id: u64,
}

impl Project {
    pub fn duplicate_nodes(
        &mut self,
        canvas_index: usize,
        selected: &std::collections::HashSet<u64>,
    ) -> Vec<u64> {
        use std::collections::HashMap;
        let Some(canvas) = self.canvases.get(canvas_index) else {
            return Vec::new();
        };
        let nodes: Vec<_> = canvas
            .nodes
            .iter()
            .filter(|n| selected.contains(&n.id))
            .cloned()
            .collect();
        let edges: Vec<_> = canvas
            .edges
            .iter()
            .filter(|e| selected.contains(&e.from) && selected.contains(&e.to))
            .cloned()
            .collect();
        let mut mapping = HashMap::new();
        let mut copies = Vec::new();
        for mut node in nodes {
            let id = self.allocate_id();
            mapping.insert(node.id, id);
            node.id = id;
            node.x += 32.0;
            node.y += 32.0;
            copies.push(id);
            self.canvases[canvas_index].nodes.push(node);
        }
        for edge in edges {
            let id = self.allocate_id();
            self.canvases[canvas_index].edges.push(Edge {
                id,
                from: mapping[&edge.from],
                to: mapping[&edge.to],
                output: edge.output,
            });
        }
        copies
    }

    pub fn validate(&self) -> Result<(), String> {
        use std::collections::HashSet;
        if self.version != 1 {
            return Err("This project format is not supported by this version of Skarva".into());
        }
        if self.name.trim().is_empty() {
            return Err("The project needs a name".into());
        }
        if self.canvases.is_empty() || self.canvases.len() > 64 {
            return Err("Projects must contain between 1 and 64 canvases".into());
        }
        let mut ids = HashSet::new();
        for canvas in &self.canvases {
            if canvas.nodes.len() > 2_000 || canvas.edges.len() > 8_000 {
                return Err("This canvas exceeds the 2,000 block or 8,000 connection limit".into());
            }
            for id in std::iter::once(canvas.id)
                .chain(canvas.nodes.iter().map(|n| n.id))
                .chain(canvas.edges.iter().map(|e| e.id))
            {
                if !ids.insert(id) {
                    return Err("The project contains duplicate IDs".into());
                }
            }
            canvas.validate()?;
        }
        if ids.iter().max().is_some_and(|id| *id >= self.next_id) {
            return Err("The project has an invalid next ID".into());
        }
        Ok(())
    }

    pub fn text_workflow(id: u64, name: impl Into<String>) -> Self {
        let mut project = Self::new(id, name);
        let mut start = Node::new(2, NodeKind::Start, 0.0, 0.0);
        start.prompt = "Explain a topic you are interested in, in a few clear sentences.".into();
        let mut model = Node::new(3, NodeKind::OpenAI, 340.0, 0.0);
        model.prompt =
            "Respond to the incoming request clearly and concisely. Return the answer directly."
                .into();
        project.canvases[0].nodes = vec![start, model, Node::new(4, NodeKind::End, 680.0, 0.0)];
        project.canvases[0].edges = vec![
            Edge {
                id: 5,
                from: 2,
                output: 0,
                to: 3,
            },
            Edge {
                id: 6,
                from: 3,
                output: 0,
                to: 4,
            },
        ];
        project.next_id = 7;
        project
    }

    pub fn remove_memory_blocks(&mut self) -> bool {
        let mut changed = false;
        for canvas in &mut self.canvases {
            let memory_ids: Vec<_> = canvas
                .nodes
                .iter()
                .filter(|n| matches!(n.kind, NodeKind::Recall | NodeKind::Remember))
                .map(|n| n.id)
                .collect();
            for id in memory_ids {
                let incoming: Vec<_> = canvas
                    .edges
                    .iter()
                    .filter(|e| e.to == id && e.from != id)
                    .cloned()
                    .collect();
                let outgoing: Vec<_> = canvas
                    .edges
                    .iter()
                    .filter(|e| e.from == id && e.to != id)
                    .cloned()
                    .collect();
                canvas.edges.retain(|e| e.from != id && e.to != id);
                canvas.nodes.retain(|n| n.id != id);
                for input in &incoming {
                    for output in &outgoing {
                        if input.from != output.to
                            && !canvas.edges.iter().any(|e| {
                                e.from == input.from
                                    && e.output == input.output
                                    && e.to == output.to
                            })
                        {
                            canvas.edges.push(Edge {
                                id: self.next_id,
                                from: input.from,
                                output: input.output,
                                to: output.to,
                            });
                            self.next_id += 1;
                        }
                    }
                }
                changed = true;
            }
        }
        changed
    }

    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            version: 1,
            id,
            name: name.into(),
            canvases: vec![Canvas::new(1, "Main canvas")],
            next_id: 2,
        }
    }

    pub fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_and_duplicate_selection_keep_valid_internal_connections() {
        let mut project = Project::text_workflow(100, "Writing");
        project.validate().unwrap();
        let copies = project.duplicate_nodes(0, &std::collections::HashSet::from([3, 4]));
        assert_eq!(copies.len(), 2);
        let canvas = &project.canvases[0];
        assert_eq!(canvas.nodes.len(), 5);
        assert_eq!(canvas.edges.len(), 3);
        let copied_edge = canvas.edges.last().unwrap();
        assert_eq!((copied_edge.from, copied_edge.to), (copies[0], copies[1]));
        assert!(
            !canvas
                .edges
                .iter()
                .any(|e| e.from == 2 && e.to == copies[0])
        );
        project.validate().unwrap();
    }

    #[test]
    fn validation_rejects_loops_broken_ports_and_bad_id_allocation() {
        let mut project = Project::text_workflow(100, "Writing");
        project.canvases[0].nodes[2].kind = NodeKind::OpenAI;
        project.canvases[0].edges.push(Edge {
            id: 7,
            from: 4,
            output: 0,
            to: 3,
        });
        project.next_id = 8;
        assert!(project.validate().unwrap_err().contains("loop"));
        project.canvases[0].edges.pop();
        project.canvases[0].edges[0].output = 99;
        assert!(project.validate().unwrap_err().contains("port"));
        project.canvases[0].edges[0].output = 0;
        project.next_id = 3;
        assert!(project.validate().unwrap_err().contains("next ID"));
        project.next_id = 8;
        project.canvases[0].nodes[0].threshold = f32::NAN;
        assert!(project.validate().is_err());
    }

    #[test]
    fn showcase_loads_with_both_inferred_entry_points() {
        let project: Project =
            serde_json::from_str(include_str!("../examples/connected-workflows.json")).unwrap();
        project.validate().unwrap();
        assert_eq!(project.canvases.len(), 2);
        assert_eq!(project.canvases[0].nodes[0].kind, NodeKind::Start);
        assert_eq!(project.canvases[1].nodes[0].kind, NodeKind::GmailInbox);
        for canvas in &project.canvases {
            assert!(
                canvas
                    .nodes
                    .iter()
                    .all(|n| !matches!(n.kind, NodeKind::Recall | NodeKind::Remember))
            );
            for edge in &canvas.edges {
                assert!(edge.output < canvas.node(edge.from).unwrap().output_count());
                assert!(canvas.node(edge.to).is_some());
            }
        }
    }

    #[test]
    fn removing_legacy_memory_preserves_routes_through_chains() {
        let mut project = Project::new(50, "Existing workflow");
        project.next_id = 20;
        let canvas = &mut project.canvases[0];
        canvas.nodes = vec![
            Node::new(2, NodeKind::Decision, 0.0, 0.0),
            Node::new(3, NodeKind::Recall, 0.0, 0.0),
            Node::new(4, NodeKind::Remember, 0.0, 0.0),
            Node::new(5, NodeKind::Word, 0.0, 0.0),
            Node::new(6, NodeKind::End, 0.0, 0.0),
        ];
        canvas.edges = vec![
            Edge {
                id: 10,
                from: 2,
                output: 1,
                to: 3,
            },
            Edge {
                id: 11,
                from: 3,
                output: 0,
                to: 4,
            },
            Edge {
                id: 12,
                from: 4,
                output: 0,
                to: 5,
            },
            Edge {
                id: 13,
                from: 2,
                output: 0,
                to: 6,
            },
        ];
        assert!(project.remove_memory_blocks());
        let canvas = &project.canvases[0];
        assert_eq!(canvas.nodes.len(), 3);
        assert_eq!(canvas.edges.len(), 2);
        assert!(
            canvas
                .edges
                .iter()
                .any(|e| e.from == 2 && e.output == 1 && e.to == 5)
        );
        assert!(
            canvas
                .edges
                .iter()
                .any(|e| e.from == 2 && e.output == 0 && e.to == 6)
        );
        assert!(canvas.edges.iter().all(|e| e.id < project.next_id));
        assert!(!project.remove_memory_blocks());
    }

    #[test]
    fn categories_cover_every_block_once() {
        for kind in NodeKind::ALL {
            assert!(BlockCategory::ALL.contains(&kind.category()));
        }
    }

    #[test]
    fn connectable_picker_excludes_entry_and_note() {
        assert!(!NodeKind::Start.accepts_input());
        assert!(!NodeKind::Note.accepts_input());
        assert!(
            NodeKind::ALL
                .into_iter()
                .any(|kind| { kind.category() == BlockCategory::Messages && kind.accepts_input() })
        );
    }

    #[test]
    fn existing_decision_without_question_uses_its_name() {
        let mut node = serde_json::to_value(Node::new(1, NodeKind::Decision, 0.0, 0.0)).unwrap();
        node.as_object_mut().unwrap().remove("decision_question");
        node["title"] = "Is this an assignment for an assistant?".into();
        let loaded: Node = serde_json::from_value(node).unwrap();
        assert_eq!(
            loaded.routing_question(),
            "Is this an assignment for an assistant?"
        );
    }

    #[test]
    fn old_decision_routes_and_new_yes_no_outputs_survive_loading() {
        let mut saved = serde_json::to_value(Node::new(1, NodeKind::Decision, 0.0, 0.0)).unwrap();
        saved.as_object_mut().unwrap().remove("mock_target");
        saved["routes"] = serde_json::json!(["Yes", "No"]);
        let loaded: Node = serde_json::from_value(saved).unwrap();
        assert_eq!(loaded.output_count(), 2);
        assert_eq!(loaded.output_label(0), "Yes");
        assert_eq!(loaded.mock_target, 0);

        let yes_no = Node::new(2, NodeKind::YesNo, 0.0, 0.0);
        assert_eq!(yes_no.output_count(), 2);
        assert_eq!(yes_no.output_label(1), "No");
    }
}
