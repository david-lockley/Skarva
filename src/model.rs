use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    Start,
    Decision,
    Copilot,
    OpenAI,
    Teams,
    Outlook,
    Excel,
    Word,
    PowerPoint,
    Note,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockCategory {
    Flow,
    Messages,
    Ai,
    Office,
}

impl BlockCategory {
    pub const ALL: [Self; 4] = [Self::Flow, Self::Messages, Self::Ai, Self::Office];

    pub fn label(self) -> &'static str {
        match self {
            Self::Flow => "Flow & canvas",
            Self::Messages => "Messages",
            Self::Ai => "AI & assistants",
            Self::Office => "Office apps",
        }
    }
}

impl NodeKind {
    pub const ALL: [Self; 11] = [
        Self::Start,
        Self::Decision,
        Self::Copilot,
        Self::OpenAI,
        Self::Teams,
        Self::Outlook,
        Self::Excel,
        Self::Word,
        Self::PowerPoint,
        Self::Note,
        Self::End,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Decision => "Decision",
            Self::Copilot => "Copilot",
            Self::OpenAI => "OpenAI",
            Self::Teams => "Teams",
            Self::Outlook => "Outlook",
            Self::Excel => "Excel",
            Self::Word => "Word",
            Self::PowerPoint => "PowerPoint",
            Self::Note => "Note",
            Self::End => "End",
        }
    }

    pub fn category(self) -> BlockCategory {
        match self {
            Self::Start | Self::Decision | Self::Note | Self::End => BlockCategory::Flow,
            Self::Teams | Self::Outlook => BlockCategory::Messages,
            Self::Copilot | Self::OpenAI => BlockCategory::Ai,
            Self::Excel | Self::Word | Self::PowerPoint => BlockCategory::Office,
        }
    }

    pub fn picker_detail(self) -> &'static str {
        match self {
            Self::Start => "Manual entry point",
            Self::Decision => "Choose one route",
            Self::Note => "Canvas annotation",
            Self::End => "Finish a path",
            Self::Copilot => "Assistant step · prototype",
            Self::OpenAI => "Model call · prototype",
            Self::Teams => "Message step · prototype",
            Self::Outlook => "Email step · prototype",
            Self::Excel => "Spreadsheet step · prototype",
            Self::Word => "Document step · prototype",
            Self::PowerPoint => "Slides step · prototype",
        }
    }

    pub fn default_subtitle(self) -> &'static str {
        match self {
            Self::Start => "Manual trigger",
            Self::Decision => "Choose a route",
            Self::Copilot => "Prototype · no connection",
            Self::OpenAI => "Prototype · no connection",
            Self::Teams => "Prototype · no connection",
            Self::Outlook => "Prototype · no connection",
            Self::Excel => "Prototype · no connection",
            Self::Word => "Prototype · no connection",
            Self::PowerPoint => "Prototype · no connection",
            Self::Note => "Canvas annotation",
            Self::End => "Finish run",
        }
    }

    pub fn outputs(self) -> &'static [&'static str] {
        match self {
            Self::Decision => &["Route A", "Route B", "Route C"],
            Self::End | Self::Note => &[],
            _ => &["Next"],
        }
    }

    pub fn accepts_input(self) -> bool {
        !matches!(self, Self::Start | Self::Note)
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
    pub routes: Vec<String>,
    #[serde(default = "default_threshold")]
    pub threshold: f32,
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
            routes,
            threshold: default_threshold(),
        }
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
}
